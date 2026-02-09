use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum EfCommandType {
    AddMigration,
    UpdateDatabase,
    RemoveMigration,
    GenerateSqlScript,
    DropDatabase,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EfCommandRequest {
    profile_id: String,
    command_type: EfCommandType,
    project_path: String,
    startup_project_path: String,
    context: Option<String>,
    migration_name: Option<String>,
    target_migration: Option<String>,
    from_migration: Option<String>,
    to_migration: Option<String>,
    output: Option<String>,
    output_dir: Option<String>,
    namespace: Option<String>,
    connection: Option<String>,
    framework: Option<String>,
    configuration: Option<String>,
    runtime: Option<String>,
    no_build: Option<bool>,
    verbose: Option<bool>,
    idempotent: Option<bool>,
    no_transactions: Option<bool>,
    force: Option<bool>,
    dry_run: Option<bool>,
    additional_args: Option<Vec<String>>,
    forwarded_args: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandPreview {
    command: String,
    args: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandExecutionResult {
    command: String,
    success: bool,
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u128,
}

const EF_COMMAND_OUTPUT_EVENT: &str = "ef-command-output";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CommandOutputChunk {
    stream: String,
    chunk: String,
}

static RUNNING_PROCESS_IDS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

fn running_process_ids() -> &'static Mutex<HashMap<String, u32>> {
    RUNNING_PROCESS_IDS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_running_process(window_label: &str, pid: u32) -> Result<(), String> {
    let mut guard = running_process_ids()
        .lock()
        .map_err(|error| format!("failed to lock running process map: {error}"))?;
    guard.insert(window_label.to_string(), pid);
    Ok(())
}

fn unregister_running_process(window_label: &str) {
    if let Ok(mut guard) = running_process_ids().lock() {
        guard.remove(window_label);
    }
}

fn find_running_process(window_label: &str) -> Option<u32> {
    running_process_ids()
        .lock()
        .ok()
        .and_then(|guard| guard.get(window_label).copied())
}

struct RunningProcessGuard {
    window_label: String,
}

impl RunningProcessGuard {
    fn new(window_label: String, pid: u32) -> Result<Self, String> {
        register_running_process(&window_label, pid)?;
        Ok(Self { window_label })
    }
}

impl Drop for RunningProcessGuard {
    fn drop(&mut self) {
        unregister_running_process(&self.window_label);
    }
}

#[cfg(target_os = "windows")]
fn terminate_process(pid: u32) -> Result<(), String> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .map_err(|error| format!("failed to run taskkill: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let payload = if stderr.is_empty() { stdout } else { stderr };
    Err(if payload.is_empty() {
        format!("failed to terminate process {pid}")
    } else {
        payload
    })
}

#[cfg(not(target_os = "windows"))]
fn terminate_process(pid: u32) -> Result<(), String> {
    let output = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output()
        .map_err(|error| format!("failed to run kill: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let payload = if stderr.is_empty() { stdout } else { stderr };
    Err(if payload.is_empty() {
        format!("failed to terminate process {pid}")
    } else {
        payload
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCheck {
    available: bool,
    output: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentStatus {
    dotnet: ToolCheck,
    dotnet_ef: ToolCheck,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthStatus {
    app: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanWorkspaceRequest {
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceScanResult {
    workspace_root: String,
    solution_path: Option<String>,
    projects: Vec<WorkspaceProjectInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceProjectInfo {
    name: String,
    path: String,
    relative_path: String,
    directory: String,
    target_frameworks: Vec<String>,
    package_references: Vec<String>,
    is_startup_candidate: bool,
    is_migrations_candidate: bool,
    db_contexts: Vec<String>,
    migration_directories: Vec<String>,
    migration_names: Vec<String>,
}

fn non_empty_trimmed(value: &str, field_name: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field_name} is required"));
    }
    Ok(trimmed.to_string())
}

fn normalize_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn to_unix_slash(value: &str) -> String {
    value.replace('\\', "/")
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|item| item.to_str())
        .map(|name| {
            matches!(
                name,
                "bin" | "obj" | ".git" | ".idea" | ".vs" | ".vscode" | "node_modules" | "target"
            )
        })
        .unwrap_or(false)
}

fn collect_files_by_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();

            if file_type.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if file_type.is_file()
                && path
                    .extension()
                    .and_then(|item| item.to_str())
                    .map(|item| item.eq_ignore_ascii_case(extension))
                    .unwrap_or(false)
            {
                files.push(path);
            }
        }
    }

    files
}

fn extract_xml_tag_values(content: &str, tag: &str) -> Vec<String> {
    let open_tag = format!("<{tag}>");
    let close_tag = format!("</{tag}>");
    let mut values = Vec::new();
    let mut rest = content;

    loop {
        let Some(start_idx) = rest.find(&open_tag) else {
            break;
        };
        let value_start = start_idx + open_tag.len();
        let after_open = &rest[value_start..];
        let Some(end_rel_idx) = after_open.find(&close_tag) else {
            break;
        };

        let value = after_open[..end_rel_idx].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }

        let next_start = value_start + end_rel_idx + close_tag.len();
        rest = &rest[next_start..];
    }

    values
}

fn extract_attribute_values(content: &str, element: &str, attribute: &str) -> Vec<String> {
    let element_head = format!("<{element}");
    let attr_head = format!("{attribute}=\"");
    let mut values = Vec::new();
    let mut rest = content;

    loop {
        let Some(start_idx) = rest.find(&element_head) else {
            break;
        };
        let after_element = &rest[start_idx + element_head.len()..];
        let Some(end_tag_idx) = after_element.find('>') else {
            break;
        };
        let element_segment = &after_element[..end_tag_idx];

        if let Some(attr_start) = element_segment.find(&attr_head) {
            let value_start = attr_start + attr_head.len();
            let attr_rest = &element_segment[value_start..];
            if let Some(value_end) = attr_rest.find('"') {
                let value = attr_rest[..value_end].trim();
                if !value.is_empty() {
                    values.push(value.to_string());
                }
            }
        }

        rest = &after_element[end_tag_idx + 1..];
    }

    values
}

fn extract_project_sdk(content: &str) -> Option<String> {
    let project_start = content.find("<Project")?;
    let after_project = &content[project_start + "<Project".len()..];
    let tag_end = after_project.find('>')?;
    let tag_body = &after_project[..tag_end];
    let sdk_key = "Sdk=\"";
    let sdk_start = tag_body.find(sdk_key)? + sdk_key.len();
    let sdk_rest = &tag_body[sdk_start..];
    let sdk_end = sdk_rest.find('"')?;
    let sdk = sdk_rest[..sdk_end].trim();
    if sdk.is_empty() {
        None
    } else {
        Some(sdk.to_string())
    }
}

fn parse_target_frameworks(content: &str) -> Vec<String> {
    let mut target_frameworks = BTreeSet::new();

    for value in extract_xml_tag_values(content, "TargetFramework") {
        if !value.trim().is_empty() {
            target_frameworks.insert(value.trim().to_string());
        }
    }

    for value in extract_xml_tag_values(content, "TargetFrameworks") {
        for framework in value.split(';') {
            let trimmed = framework.trim();
            if !trimmed.is_empty() {
                target_frameworks.insert(trimmed.to_string());
            }
        }
    }

    target_frameworks.into_iter().collect()
}

fn parse_package_references(content: &str) -> Vec<String> {
    let mut packages = BTreeSet::new();

    for value in extract_attribute_values(content, "PackageReference", "Include") {
        packages.insert(value);
    }

    for value in extract_attribute_values(content, "PackageReference", "Update") {
        packages.insert(value);
    }

    packages.into_iter().collect()
}

fn parse_major_minor(version: &str) -> Option<(u32, u32)> {
    let numeric = version
        .chars()
        .take_while(|item| item.is_ascii_digit() || *item == '.')
        .collect::<String>();

    if numeric.is_empty() {
        return None;
    }

    let mut parts = numeric.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    Some((major, minor))
}

fn is_tfm_netcoreapp_at_least(tfm: &str, expected_major: u32, expected_minor: u32) -> bool {
    let value = tfm.to_ascii_lowercase();
    if !value.starts_with("netcoreapp") {
        return false;
    }

    parse_major_minor(value.trim_start_matches("netcoreapp"))
        .map(|(major, minor)| (major, minor) >= (expected_major, expected_minor))
        .unwrap_or(false)
}

fn is_tfm_netstandard_at_least(tfm: &str, expected_major: u32, expected_minor: u32) -> bool {
    let value = tfm.to_ascii_lowercase();
    if !value.starts_with("netstandard") {
        return false;
    }

    parse_major_minor(value.trim_start_matches("netstandard"))
        .map(|(major, minor)| (major, minor) >= (expected_major, expected_minor))
        .unwrap_or(false)
}

fn is_modern_dotnet_tfm(tfm: &str) -> bool {
    let value = tfm.to_ascii_lowercase();
    if value.starts_with("netcoreapp")
        || value.starts_with("netstandard")
        || !value.starts_with("net")
    {
        return false;
    }

    parse_major_minor(value.trim_start_matches("net"))
        .map(|(major, _)| major >= 5)
        .unwrap_or(false)
}

fn supports_startup_framework(target_frameworks: &[String]) -> bool {
    if target_frameworks.is_empty() {
        return true;
    }

    target_frameworks
        .iter()
        .any(|tfm| is_tfm_netcoreapp_at_least(tfm, 3, 1) || is_modern_dotnet_tfm(tfm))
}

fn supports_migrations_framework(target_frameworks: &[String]) -> bool {
    if target_frameworks.is_empty() {
        return true;
    }

    target_frameworks.iter().any(|tfm| {
        is_tfm_netcoreapp_at_least(tfm, 3, 1)
            || is_modern_dotnet_tfm(tfm)
            || is_tfm_netstandard_at_least(tfm, 2, 0)
    })
}

fn extract_namespace(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("namespace ") {
            let raw = trimmed.trim_start_matches("namespace ").trim();
            let namespace = raw.trim_end_matches('{').trim_end_matches(';').trim();
            if !namespace.is_empty() {
                return Some(namespace.to_string());
            }
        }
    }
    None
}

fn collect_class_declarations(content: &str) -> Vec<String> {
    let mut declarations = Vec::new();
    let mut buffer = String::new();
    let mut collecting = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if !collecting {
            if trimmed.contains("class ") {
                buffer.clear();
                buffer.push_str(trimmed);
                if trimmed.contains('{') || trimmed.ends_with(';') {
                    declarations.push(buffer.clone());
                } else {
                    collecting = true;
                }
            }
            continue;
        }

        buffer.push(' ');
        buffer.push_str(trimmed);
        if trimmed.contains('{') || trimmed.ends_with(';') {
            declarations.push(buffer.clone());
            collecting = false;
        }
    }

    if collecting && !buffer.is_empty() {
        declarations.push(buffer);
    }

    declarations
}

fn extract_class_name(declaration: &str) -> Option<String> {
    let class_idx = declaration.find("class ")?;
    let after_class = &declaration[class_idx + "class ".len()..];
    let mut name = String::new();

    for ch in after_class.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(ch);
        } else {
            break;
        }
    }

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn extract_base_type_names(declaration: &str) -> Vec<String> {
    let Some(colon_idx) = declaration.find(':') else {
        return Vec::new();
    };
    let end_idx = declaration.find('{').unwrap_or(declaration.len());
    let inheritance = &declaration[colon_idx + 1..end_idx];

    inheritance
        .split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return None;
            }

            let type_token = trimmed
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .split('<')
                .next()
                .unwrap_or_default()
                .trim();

            if type_token.is_empty() {
                return None;
            }

            let simple_name = type_token.rsplit('.').next().unwrap_or(type_token).trim();

            if simple_name.is_empty() {
                None
            } else {
                Some(simple_name.to_string())
            }
        })
        .collect()
}

fn scan_csharp_content(content: &str) -> (BTreeSet<String>, bool) {
    let namespace = extract_namespace(content);
    let mut db_contexts = BTreeSet::new();
    let mut contains_migration = false;

    for declaration in collect_class_declarations(content) {
        let Some(class_name) = extract_class_name(&declaration) else {
            continue;
        };
        let base_type_names = extract_base_type_names(&declaration);

        if base_type_names
            .iter()
            .any(|base| base == "DbContext" || base.ends_with("DbContext"))
        {
            let full_name = namespace
                .as_ref()
                .map(|item| format!("{item}.{class_name}"))
                .unwrap_or(class_name.clone());
            db_contexts.insert(full_name);
        }

        if base_type_names.iter().any(|base| base == "Migration") {
            contains_migration = true;
        }
    }

    (db_contexts, contains_migration)
}

fn looks_like_timestamped_migration_stem(stem: &str) -> bool {
    let bytes = stem.as_bytes();
    if bytes.len() < 16 {
        return false;
    }

    if bytes[14] != b'_' {
        return false;
    }

    bytes[..14].iter().all(|item| item.is_ascii_digit())
}

fn should_skip_migration_filename(stem: &str) -> bool {
    let lower = stem.to_ascii_lowercase();
    lower.ends_with(".designer") || lower.ends_with("modelsnapshot")
}
fn parse_solution_project_paths(solution_path: &Path) -> Vec<PathBuf> {
    let solution_dir = solution_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let content = fs::read_to_string(solution_path).unwrap_or_default();
    let mut project_paths = BTreeSet::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Project(") || !trimmed.to_ascii_lowercase().contains(".csproj") {
            continue;
        }

        let Some(eq_idx) = trimmed.find('=') else {
            continue;
        };
        let after_eq = trimmed[eq_idx + 1..].trim();
        let Some(first_comma_idx) = after_eq.find(',') else {
            continue;
        };
        let after_first_comma = after_eq[first_comma_idx + 1..].trim();
        if !after_first_comma.starts_with('"') {
            continue;
        }
        let quoted_rest = &after_first_comma[1..];
        let Some(closing_quote_idx) = quoted_rest.find('"') else {
            continue;
        };

        let raw_path = &quoted_rest[..closing_quote_idx];
        if !raw_path.to_ascii_lowercase().ends_with(".csproj") {
            continue;
        }

        let normalized = to_unix_slash(raw_path);
        let candidate = if Path::new(&normalized).is_absolute() {
            PathBuf::from(normalized)
        } else {
            solution_dir.join(normalized)
        };
        if candidate.exists() {
            project_paths.insert(normalize_path(&candidate));
        }
    }

    project_paths.into_iter().collect()
}

fn discover_workspace(path: &Path) -> Result<(PathBuf, Option<PathBuf>, Vec<PathBuf>), String> {
    if !path.exists() {
        return Err("workspace path not found".to_string());
    }

    if path.is_file() {
        let file_ext = path
            .extension()
            .and_then(|item| item.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if file_ext == "sln" {
            let solution_path = normalize_path(path);
            let workspace_root = solution_path
                .parent()
                .map(normalize_path)
                .ok_or_else(|| "invalid solution path".to_string())?;

            let mut projects = parse_solution_project_paths(&solution_path);
            if projects.is_empty() {
                projects = collect_files_by_extension(&workspace_root, "csproj")
                    .into_iter()
                    .map(|item| normalize_path(&item))
                    .collect();
            }

            projects.sort();
            projects.dedup();

            return Ok((workspace_root, Some(solution_path), projects));
        }

        if file_ext == "csproj" {
            let project_path = normalize_path(path);
            let workspace_root = project_path
                .parent()
                .map(normalize_path)
                .ok_or_else(|| "invalid project path".to_string())?;
            return Ok((workspace_root, None, vec![project_path]));
        }

        return Err("path must be a directory, .sln, or .csproj file".to_string());
    }

    if !path.is_dir() {
        return Err("workspace path is not a directory".to_string());
    }

    let workspace_root = normalize_path(path);
    let mut projects = collect_files_by_extension(&workspace_root, "csproj")
        .into_iter()
        .map(|item| normalize_path(&item))
        .collect::<Vec<PathBuf>>();
    projects.sort();
    projects.dedup();
    Ok((workspace_root, None, projects))
}

fn analyze_project(project_path: &Path, workspace_root: &Path) -> WorkspaceProjectInfo {
    let project_content = fs::read_to_string(project_path).unwrap_or_default();
    let target_frameworks = parse_target_frameworks(&project_content);
    let package_references = parse_package_references(&project_content);
    let project_sdk = extract_project_sdk(&project_content).unwrap_or_default();
    let output_type = extract_xml_tag_values(&project_content, "OutputType")
        .into_iter()
        .next()
        .unwrap_or_default();

    let has_design_or_tools = package_references.iter().any(|item| {
        item.eq_ignore_ascii_case("Microsoft.EntityFrameworkCore.Design")
            || item.eq_ignore_ascii_case("Microsoft.EntityFrameworkCore.Tools")
    });
    let has_efcore_package = package_references.iter().any(|item| {
        item.to_ascii_lowercase()
            .starts_with("microsoft.entityframeworkcore")
    });

    let is_web_sdk = project_sdk.eq_ignore_ascii_case("Microsoft.NET.Sdk.Web");
    let is_executable =
        output_type.eq_ignore_ascii_case("Exe") || output_type.eq_ignore_ascii_case("WinExe");

    let is_startup_candidate = supports_startup_framework(&target_frameworks)
        && (is_web_sdk || is_executable || has_design_or_tools || has_efcore_package);
    let is_migrations_candidate =
        supports_migrations_framework(&target_frameworks) && has_efcore_package;

    let project_dir = project_path.parent().unwrap_or_else(|| Path::new("."));

    let mut db_contexts = BTreeSet::new();
    let mut migration_directories = BTreeSet::new();
    let mut migration_names = BTreeSet::new();

    for source_file in collect_files_by_extension(project_dir, "cs") {
        let Ok(content) = fs::read_to_string(&source_file) else {
            continue;
        };

        let (detected_contexts, contains_migration_class) = scan_csharp_content(&content);
        db_contexts.extend(detected_contexts);

        let file_stem = source_file
            .file_stem()
            .and_then(|item| item.to_str())
            .unwrap_or_default()
            .to_string();
        let is_timestamped_name = looks_like_timestamped_migration_stem(&file_stem);
        let is_migration_file = contains_migration_class || is_timestamped_name;

        if !is_migration_file {
            continue;
        }

        let relative_dir = source_file
            .parent()
            .and_then(|dir| dir.strip_prefix(project_dir).ok())
            .map(|item| to_unix_slash(&item.to_string_lossy()))
            .unwrap_or_default();

        let effective_dir = if relative_dir.is_empty() {
            "Migrations".to_string()
        } else {
            relative_dir
        };
        migration_directories.insert(effective_dir);

        if !file_stem.is_empty() && !should_skip_migration_filename(&file_stem) {
            migration_names.insert(file_stem);
        }
    }

    if migration_directories.is_empty() {
        migration_directories.insert("Migrations".to_string());
    }

    let relative_path = project_path
        .strip_prefix(workspace_root)
        .map(|item| to_unix_slash(&item.to_string_lossy()))
        .unwrap_or_else(|_| {
            project_path
                .file_name()
                .and_then(|item| item.to_str())
                .map(|item| item.to_string())
                .unwrap_or_else(|| path_to_string(project_path))
        });

    WorkspaceProjectInfo {
        name: project_path
            .file_stem()
            .and_then(|item| item.to_str())
            .unwrap_or("UnknownProject")
            .to_string(),
        path: path_to_string(project_path),
        relative_path,
        directory: path_to_string(project_dir),
        target_frameworks,
        package_references,
        is_startup_candidate,
        is_migrations_candidate,
        db_contexts: db_contexts.into_iter().collect(),
        migration_directories: migration_directories.into_iter().collect(),
        migration_names: migration_names.into_iter().collect(),
    }
}
fn normalized_optional(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn push_flag(args: &mut Vec<String>, enabled: Option<bool>, flag: &str) {
    if enabled.unwrap_or(false) {
        args.push(flag.to_string());
    }
}

fn push_option(args: &mut Vec<String>, name: &str, value: &Option<String>) {
    if let Some(parsed) = normalized_optional(value) {
        args.push(name.to_string());
        args.push(parsed);
    }
}

fn build_dotnet_args(request: &EfCommandRequest) -> Result<Vec<String>, String> {
    let mut args = vec!["ef".to_string()];

    let _profile_id = non_empty_trimmed(&request.profile_id, "profileId")?;

    let project_path = non_empty_trimmed(&request.project_path, "projectPath")?;
    let startup_project_path =
        non_empty_trimmed(&request.startup_project_path, "startupProjectPath")?;

    match request.command_type {
        EfCommandType::AddMigration => {
            let migration_name = non_empty_trimmed(
                request.migration_name.as_deref().unwrap_or_default(),
                "migrationName",
            )?;
            args.extend(["migrations".to_string(), "add".to_string(), migration_name]);
            push_option(&mut args, "--output-dir", &request.output_dir);
            push_option(&mut args, "--namespace", &request.namespace);
        }
        EfCommandType::UpdateDatabase => {
            args.extend(["database".to_string(), "update".to_string()]);
            if let Some(target_migration) = normalized_optional(&request.target_migration) {
                args.push(target_migration);
            }
            push_option(&mut args, "--connection", &request.connection);
        }
        EfCommandType::RemoveMigration => {
            args.extend(["migrations".to_string(), "remove".to_string()]);
            push_flag(&mut args, request.force, "--force");
        }
        EfCommandType::GenerateSqlScript => {
            args.extend(["migrations".to_string(), "script".to_string()]);
            if let Some(from_migration) = normalized_optional(&request.from_migration) {
                args.push(from_migration);
            }
            if let Some(to_migration) = normalized_optional(&request.to_migration) {
                args.push(to_migration);
            }
            push_option(&mut args, "--output", &request.output);
            push_flag(&mut args, request.idempotent, "--idempotent");
            push_flag(&mut args, request.no_transactions, "--no-transactions");
        }
        EfCommandType::DropDatabase => {
            args.extend(["database".to_string(), "drop".to_string()]);
            push_flag(&mut args, request.force, "--force");
            push_flag(&mut args, request.dry_run, "--dry-run");
        }
    }

    push_option(
        &mut args,
        "--context",
        &normalized_optional(&request.context),
    );
    args.push("--project".to_string());
    args.push(project_path);
    args.push("--startup-project".to_string());
    args.push(startup_project_path);

    push_option(
        &mut args,
        "--framework",
        &normalized_optional(&request.framework),
    );
    push_option(
        &mut args,
        "--configuration",
        &normalized_optional(&request.configuration),
    );
    push_option(
        &mut args,
        "--runtime",
        &normalized_optional(&request.runtime),
    );
    push_flag(&mut args, request.no_build, "--no-build");
    push_flag(&mut args, request.verbose, "--verbose");

    if let Some(additional_args) = &request.additional_args {
        for arg in additional_args {
            if !arg.trim().is_empty() {
                args.push(arg.trim().to_string());
            }
        }
    }

    if let Some(forwarded_args) = &request.forwarded_args {
        let filtered: Vec<String> = forwarded_args
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();

        if !filtered.is_empty() {
            args.push("--".to_string());
            args.extend(filtered);
        }
    }

    Ok(args)
}

fn shell_escape(value: &str) -> String {
    if value.contains(' ') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn build_preview(args: &[String]) -> String {
    let rendered = args
        .iter()
        .map(|item| shell_escape(item))
        .collect::<Vec<String>>()
        .join(" ");
    format!("dotnet {rendered}")
}

fn emit_command_output(app: &tauri::AppHandle, window_label: &str, stream: &str, chunk: String) {
    let payload = CommandOutputChunk {
        stream: stream.to_string(),
        chunk,
    };
    let _ = app.emit_to(window_label, EF_COMMAND_OUTPUT_EVENT, payload);
}

fn flush_pending_command_output(
    app: &tauri::AppHandle,
    window_label: &str,
    stdout_pending: &mut String,
    stderr_pending: &mut String,
) {
    if !stdout_pending.is_empty() {
        emit_command_output(app, window_label, "stdout", std::mem::take(stdout_pending));
    }

    if !stderr_pending.is_empty() {
        emit_command_output(app, window_label, "stderr", std::mem::take(stderr_pending));
    }
}

fn spawn_output_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    stream: &'static str,
    tx: mpsc::Sender<(&'static str, String)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let buffered_reader = BufReader::new(reader);
        for line_result in buffered_reader.lines() {
            let Ok(line) = line_result else {
                break;
            };

            let mut chunk = line;
            chunk.push('\n');

            if tx.send((stream, chunk)).is_err() {
                break;
            }
        }
    })
}

fn execute_ef_command_streaming(
    app: tauri::AppHandle,
    window_label: String,
    request: EfCommandRequest,
) -> Result<CommandExecutionResult, String> {
    let args = build_dotnet_args(&request)?;
    let command = build_preview(&args);

    let started = Instant::now();
    let mut child = Command::new("dotnet")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to execute dotnet: {error}"))?;

    let _running_process_guard = RunningProcessGuard::new(window_label.clone(), child.id())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture dotnet stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture dotnet stderr".to_string())?;

    let (tx, rx) = mpsc::channel::<(&'static str, String)>();
    let stdout_reader = spawn_output_reader(stdout, "stdout", tx.clone());
    let stderr_reader = spawn_output_reader(stderr, "stderr", tx.clone());
    drop(tx);

    let mut stdout_buffer = String::new();
    let mut stderr_buffer = String::new();
    let mut stdout_pending = String::new();
    let mut stderr_pending = String::new();
    let mut last_emit = Instant::now();

    for (stream, chunk) in rx {
        if stream == "stderr" {
            stderr_buffer.push_str(&chunk);
            stderr_pending.push_str(&chunk);
        } else {
            stdout_buffer.push_str(&chunk);
            stdout_pending.push_str(&chunk);
        }

        let pending_len = stdout_pending.len() + stderr_pending.len();
        if pending_len >= 4096 || last_emit.elapsed() >= Duration::from_millis(50) {
            flush_pending_command_output(
                &app,
                &window_label,
                &mut stdout_pending,
                &mut stderr_pending,
            );
            last_emit = Instant::now();
        }
    }

    flush_pending_command_output(
        &app,
        &window_label,
        &mut stdout_pending,
        &mut stderr_pending,
    );

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();

    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for dotnet: {error}"))?;
    let duration_ms = started.elapsed().as_millis();

    Ok(CommandExecutionResult {
        command,
        success: status.success(),
        exit_code: status.code().unwrap_or(-1),
        stdout: stdout_buffer,
        stderr: stderr_buffer,
        duration_ms,
    })
}

fn run_tool_check(program: &str, args: &[&str]) -> ToolCheck {
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let success = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let payload = if stdout.trim().is_empty() {
                stderr
            } else {
                stdout
            };

            ToolCheck {
                available: success,
                output: payload.trim().to_string(),
            }
        }
        Err(error) => ToolCheck {
            available: false,
            output: error.to_string(),
        },
    }
}

#[tauri::command]
fn scan_workspace(request: ScanWorkspaceRequest) -> Result<WorkspaceScanResult, String> {
    let normalized_input = non_empty_trimmed(&request.path, "path")?;
    let input_path = PathBuf::from(normalized_input);

    let (workspace_root, solution_path, project_paths) = discover_workspace(&input_path)?;
    let mut projects = project_paths
        .iter()
        .map(|item| analyze_project(item, &workspace_root))
        .collect::<Vec<WorkspaceProjectInfo>>();

    projects.sort_by(|a, b| {
        a.relative_path
            .cmp(&b.relative_path)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });

    Ok(WorkspaceScanResult {
        workspace_root: path_to_string(&workspace_root),
        solution_path: solution_path.map(|item| path_to_string(&item)),
        projects,
    })
}

#[tauri::command]
fn health_check() -> HealthStatus {
    HealthStatus {
        app: "EfCorePilot".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
fn detect_environment() -> EnvironmentStatus {
    EnvironmentStatus {
        dotnet: run_tool_check("dotnet", &["--version"]),
        dotnet_ef: run_tool_check("dotnet", &["ef", "--version"]),
    }
}

#[tauri::command]
fn preview_ef_command(request: EfCommandRequest) -> Result<CommandPreview, String> {
    let args = build_dotnet_args(&request)?;
    let command = build_preview(&args);
    Ok(CommandPreview { command, args })
}

#[tauri::command]
async fn execute_ef_command(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    request: EfCommandRequest,
) -> Result<CommandExecutionResult, String> {
    let window_label = window.label().to_string();

    tauri::async_runtime::spawn_blocking(move || {
        execute_ef_command_streaming(app, window_label, request)
    })
    .await
    .map_err(|error| format!("failed to join dotnet execution task: {error}"))?
}

#[tauri::command]
fn interrupt_ef_command(window: tauri::WebviewWindow) -> Result<bool, String> {
    let window_label = window.label().to_string();
    let Some(pid) = find_running_process(&window_label) else {
        return Ok(false);
    };

    terminate_process(pid)?;
    Ok(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            health_check,
            detect_environment,
            scan_workspace,
            preview_ef_command,
            execute_ef_command,
            interrupt_ef_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

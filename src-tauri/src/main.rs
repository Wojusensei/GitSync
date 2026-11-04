#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::{Oid, Repository, Sort};
use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::DialogExt;

// 只允许普通组件组成的相对路径，拒绝绝对路径和 ..
fn safe_relative_path(untrusted: &str) -> Result<PathBuf, String> {
    let path = Path::new(untrusted);
    if path.is_absolute() {
        return Err(format!("拒绝绝对路径: {}", untrusted));
    }
    let mut cleaned = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(part) => cleaned.push(part),
            _ => return Err(format!("路径包含非法组件: {}", untrusted)),
        }
    }
    if cleaned.as_os_str().is_empty() {
        return Err("路径不能为空".to_string());
    }
    Ok(cleaned)
}

// canonicalize 后确认仍在 base 内，防符号链接穿越
fn resolve_existing_under(base: &Path, relative: &Path) -> Result<PathBuf, String> {
    let base_canonical = base
        .canonicalize()
        .map_err(|e| format!("无法解析目录 {}: {}", base.display(), e))?;
    let target = base.join(relative);
    let canonical = target
        .canonicalize()
        .map_err(|e| format!("路径无效: {}", e))?;
    if !canonical.starts_with(&base_canonical) {
        return Err("拒绝访问目标目录之外的文件".into());
    }
    Ok(canonical)
}

#[derive(Serialize, Clone)]
struct Commit {
    hash: String,
    author: String,
    time: String,
    message: String,
}

#[derive(Serialize)]
struct Branch {
    name: String,
    is_head: bool,
}

#[derive(Serialize)]
struct CommitDetail {
    hash: String,
    author: String,
    time: String,
    message: String,
    files: Vec<FileChange>,
}

#[derive(Serialize)]
struct FileChange {
    path: String,
    status: String,
    additions: usize,
    deletions: usize,
    diff: String,
}

// 各入口的提交遍历上限
const MAIN_COMMIT_LIMIT: usize = 100;
const ANALYSIS_COMMIT_LIMIT: usize = 2000;
const QUERY_COMMIT_LIMIT: usize = 500;

// 本地时区时间，超出 chrono 范围回退「未知时间」
fn format_local_time(seconds: i64) -> String {
    chrono::Local
        .timestamp_opt(seconds, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "未知时间".into())
}

fn commit_to_json(oid: Oid, commit: &git2::Commit<'_>) -> Commit {
    let time = commit.time();
    let timestamp = format_local_time(time.seconds());
    Commit {
        hash: oid.to_string(),
        author: commit.author().name().unwrap_or("未知").to_string(),
        time: timestamp,
        message: commit.message().unwrap_or("").to_string(),
    }
}

// TIME|TOPOLOGICAL：提交时钟偏斜时仅按时间排序会出现父提交排在子提交之后
fn sorted_revwalk(repo: &Repository) -> Result<git2::Revwalk<'_>, String> {
    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk
        .set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(|e| format!("无法设置排序: {}", e))?;
    Ok(revwalk)
}

// all_refs 时遍历全部分支/标签/stash，peel 到提交；默认只看 HEAD
fn sorted_revwalk_refs(repo: &Repository, all_refs: bool) -> Result<git2::Revwalk<'_>, String> {
    if !all_refs {
        return sorted_revwalk(repo);
    }
    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    let refs = repo.references().map_err(|e| format!("无法获取引用: {}", e))?;
    let mut pushed = false;
    for reference in refs.flatten() {
        if let Ok(commit) = reference.peel_to_commit() {
            let _ = revwalk.push(commit.id());
            pushed = true;
        }
    }
    if !pushed {
        return Err("仓库没有任何可遍历的引用".into());
    }
    revwalk
        .set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(|e| format!("无法设置排序: {}", e))?;
    Ok(revwalk)
}

fn collect_commits(repo: &Repository, limit: usize) -> Result<Vec<Commit>, String> {
    collect_commits_refs(repo, limit, false)
}

fn collect_commits_refs(repo: &Repository, limit: usize, all_refs: bool) -> Result<Vec<Commit>, String> {
    let mut commits = Vec::new();
    for oid in sorted_revwalk_refs(repo, all_refs)? {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        commits.push(commit_to_json(oid, &commit));
        if commits.len() >= limit {
            break;
        }
    }
    Ok(commits)
}

#[tauri::command]
fn get_commits(path: String, limit: Option<usize>) -> Result<Vec<Commit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    collect_commits(&repo, limit.unwrap_or(MAIN_COMMIT_LIMIT).min(ANALYSIS_COMMIT_LIMIT))
}

#[tauri::command]
fn get_branches(path: String) -> Result<Vec<Branch>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let branches = repo
        .branches(None)
        .map_err(|e| format!("无法获取分支: {}", e))?;

    let head_name = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().ok().map(|s| s.to_string()));

    let mut result = Vec::new();
    for branch in branches {
        let (branch, _) = branch.map_err(|e| format!("分支错误: {}", e))?;
        let opt_name = branch.name().map_err(|e| format!("分支名错误: {}", e))?;
        let name = opt_name.unwrap_or("未知").to_string();
        if name == "HEAD" || name.ends_with("/HEAD") {
            continue;
        }
        let is_head = head_name.as_deref() == Some(&name);
        result.push(Branch { name, is_head });
    }

    result.sort_by(|a, b| b.is_head.cmp(&a.is_head));

    Ok(result)
}

fn resolve_switch_target(repo: &Repository, branch_name: &str) -> (String, bool) {
    if repo.find_branch(branch_name, git2::BranchType::Local).is_ok() {
        return (branch_name.to_string(), false);
    }

    if let Ok(remotes) = repo.remotes() {
        for remote in remotes.iter() {
            if let Ok(Some(remote_name)) = remote {
                if let Some(local_name) = branch_name.strip_prefix(&format!("{}/", remote_name)) {
                    let track = repo.find_branch(local_name, git2::BranchType::Local).is_err();
                    return (local_name.to_string(), track);
                }
            }
        }
    }
    (branch_name.to_string(), false)
}

#[tauri::command]
fn checkout_branch(path: String, branch_name: String) -> Result<(), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let is_current = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().ok().map(|s| s.to_string()))
        .map(|name| name == branch_name)
        .unwrap_or(false);
    if is_current {
        return Ok(());
    }

    let (target, track) = resolve_switch_target(&repo, &branch_name);

    let commit = if track {
        let remote = repo
            .find_branch(&branch_name, git2::BranchType::Remote)
            .map_err(|e| format!("找不到远程分支 {}: {}", branch_name, e))?;
        remote
            .get()
            .peel_to_commit()
            .map_err(|e| format!("无法解析远程分支: {}", e))?
    } else {
        repo.find_branch(&target, git2::BranchType::Local)
            .map_err(|e| format!("找不到本地分支 {}: {}", target, e))?
            .get()
            .peel_to_commit()
            .map_err(|e| format!("无法解析分支: {}", e))?
    };

    // 先建分支设上游，再更新工作区，最后移动 HEAD；checkout_tree 失败时工作区保持原样
    if track {
        let mut local = repo
            .branch(&target, &commit, false)
            .map_err(|e| format!("创建本地分支失败: {}", e))?;
        local
            .set_upstream(Some(&branch_name))
            .map_err(|e| format!("设置上游失败: {}", e))?;
    }

    // 预检：工作区改动与目标分支改动重叠时 safe checkout 必然失败，
    // 先列出挡路的文件，避免把 libgit2 的英文错误直接抛给用户
    let mut blocking: Vec<String> = Vec::new();
    if let Ok(target_tree) = commit.tree() {
        let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        if let Ok(diff) = repo.diff_tree_to_tree(head_tree.as_ref(), Some(&target_tree), None) {
            let changed: std::collections::HashSet<String> = diff
                .deltas()
                .filter_map(|d| d.new_file().path().or_else(|| d.old_file().path()))
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            if let Ok(statuses) = repo.statuses(None) {
                for entry in statuses.iter() {
                    let status = entry.status();
                    let dirty = status.intersects(
                        git2::Status::WT_MODIFIED
                            | git2::Status::INDEX_MODIFIED
                            | git2::Status::WT_DELETED
                            | git2::Status::INDEX_DELETED
                            | git2::Status::WT_RENAMED
                            | git2::Status::INDEX_RENAMED
                            | git2::Status::WT_NEW,
                    );
                    if dirty {
                        if let Ok(path) = entry.path() {
                            if changed.contains(path) {
                                blocking.push(path.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    if !blocking.is_empty() {
        blocking.truncate(5);
        return Err(format!(
            "工作区有未提交修改，切换会覆盖这些文件：{}。请先提交或 stash 后再切换",
            blocking.join("、")
        ));
    }

    repo.checkout_tree(&commit.as_object(), Some(&mut git2::build::CheckoutBuilder::default()))
        .map_err(|e| {
            if e.code() == git2::ErrorCode::Conflict {
                "切换分支失败：工作区存在与目标分支冲突的修改，请先提交或 stash 后再切换".to_string()
            } else {
                format!("切换分支失败: {}", e)
            }
        })?;

    repo.set_head(&format!("refs/heads/{}", target))
        .map_err(|e| format!("设置 HEAD 失败: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn get_commit_detail(path: String, commit_hash: String) -> Result<CommitDetail, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let oid = Oid::from_str(&commit_hash).map_err(|e| format!("无效的哈希: {}", e))?;
    let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
    let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;

    let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .map_err(|e| format!("无法生成 Diff: {}", e))?;

    let mut files = Vec::new();
    let deltas: Vec<git2::DiffDelta<'_>> = diff.deltas().collect();

    for (idx, delta) in deltas.iter().enumerate() {
        let status = match delta.status() {
            git2::Delta::Added => "A",
            git2::Delta::Deleted => "D",
            git2::Delta::Modified => "M",
            git2::Delta::Renamed => "R",
            _ => "?",
        };

        // 删除的文件 new_file 无路径，回退 old_file
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "未知文件".into());

        let patch = git2::Patch::from_diff(&diff, idx)
            .map_err(|e| format!("Patch 创建失败: {}", e))?;

        let (additions, deletions, diff_text) = if let Some(mut p) = patch {
            let mut add = 0;
            let mut del = 0;
            let mut diff_content = Vec::new();

            p.print(&mut |_delta, _hunk, line| {
                match line.origin() {
                    '+' => add += 1,
                    '-' => del += 1,
                    _ => {}
                }
                // 带 +/-/空格 前缀，前端据此识别增删行
                diff_content.push(line.origin() as u8);
                diff_content.extend_from_slice(line.content());
                true
            })
            .map_err(|e| format!("Diff 打印失败: {}", e))?;

            let diff_str = String::from_utf8_lossy(&diff_content).to_string();
            (add, del, diff_str)
        } else {
            (0, 0, String::new())
        };

        files.push(FileChange {
            path,
            status: status.to_string(),
            additions,
            deletions,
            diff: diff_text,
        });
    }

    let author_name = commit.author().name().unwrap_or("未知").to_string();
    let message = commit.message().unwrap_or("").to_string();
    let timestamp = format_local_time(commit.time().seconds());

    Ok(CommitDetail {
        hash: commit_hash,
        author: author_name,
        time: timestamp,
        message,
        files,
    })
}

#[tauri::command]
fn search_commits(path: String, query: String, all_refs: Option<bool>) -> Result<Vec<Commit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    let all = collect_commits_refs(&repo, ANALYSIS_COMMIT_LIMIT, all_refs.unwrap_or(false))?;
    let query_lower = query.to_lowercase();
    let filtered: Vec<Commit> = all
        .into_iter()
        .filter(|c| {
            c.message.to_lowercase().contains(&query_lower)
                || c.author.to_lowercase().contains(&query_lower)
        })
        .take(50)
        .collect();
    Ok(filtered)
}

#[derive(Serialize)]
struct BlameLine {
    line_number: usize,
    commit_hash: String,
    author: String,
    time: String,
    content: String,
}

#[tauri::command]
async fn get_blame(path: String, file_path: String) -> Result<Vec<BlameLine>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let head = repo.head().map_err(|e| format!("无法获取 HEAD: {}", e))?;
    let head_oid = head.target().ok_or("HEAD 没有指向任何提交")?;
    let commit = repo.find_commit(head_oid).map_err(|e| format!("找不到 HEAD 提交: {}", e))?;
    let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;

    let blob = tree
        .get_path(Path::new(&file_path))
        .map_err(|e| format!("找不到文件: {}", e))?;
    let blob = repo.find_blob(blob.id()).map_err(|e| format!("无法获取文件内容: {}", e))?;
    let content = std::str::from_utf8(blob.content()).unwrap_or("");
    let file_lines: Vec<&str> = content.lines().collect();

    let blame = repo
        .blame_file(Path::new(&file_path), None)
        .map_err(|e| format!("无法获取 Blame: {}", e))?;

    let mut lines = Vec::new();

    for hunk in blame.iter() {
        let final_commit_id = hunk.final_commit_id();
        let sig = hunk.final_signature();
        let author = sig.as_ref().map_or("未知", |s| {
            if let Ok(name) = s.name() { name } else { "未知" }
        });
        let time = sig
            .as_ref()
            .map_or("未知时间".into(), |s| format_local_time(s.when().seconds()));

        let start_line = hunk.final_start_line() as usize;
        let num_lines = hunk.lines_in_hunk() as usize;

        for i in 0..num_lines {
            let line_idx = start_line.saturating_add(i).saturating_sub(1);
            let content = if line_idx < file_lines.len() {
                file_lines[line_idx].to_string()
            } else {
                String::new()
            };
            lines.push(BlameLine {
                line_number: start_line + i,
                commit_hash: final_commit_id.to_string(),
                author: author.to_string(),
                time: time.clone(),
                content,
            });
        }
    }

    Ok(lines)
}

#[derive(Serialize)]
struct FileTimelineEntry {
    commit_hash: String,
    author: String,
    time: String,
    message: String,
    diff: String,
}

#[tauri::command]
async fn get_file_timeline(path: String, file_path: String) -> Result<Vec<FileTimelineEntry>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    get_file_timeline_impl(&repo, &file_path)
}

fn get_file_timeline_impl(repo: &Repository, file_path: &str) -> Result<Vec<FileTimelineEntry>, String> {
    let revwalk = sorted_revwalk(repo)?;

    let mut entries = Vec::new();

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;

        let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());

        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .map_err(|e| format!("无法生成 Diff: {}", e))?;

        let mut file_changed = false;
        let mut diff_text = String::new();

        let deltas: Vec<git2::DiffDelta<'_>> = diff.deltas().collect();
        for (idx, delta) in deltas.iter().enumerate() {
            // 删除的文件 new_file 路径为 None，回退到 old_file
            let delta_path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            if delta_path == file_path {
                file_changed = true;
                let patch = git2::Patch::from_diff(&diff, idx)
                    .map_err(|e| format!("Patch 创建失败: {}", e))?;
                if let Some(mut p) = patch {
                    let mut diff_content = Vec::new();
                    // 带 origin 前缀，与 get_commit_detail 一致
                    p.print(&mut |_delta, _hunk, line| {
                        diff_content.push(line.origin() as u8);
                        diff_content.extend_from_slice(line.content());
                        true
                    })
                    .map_err(|e| format!("Diff 打印失败: {}", e))?;
                    diff_text = String::from_utf8_lossy(&diff_content).to_string();
                }
                break;
            }
        }

        if file_changed {
            let author_name = commit.author().name().unwrap_or("未知").to_string();
            let message = commit.message().unwrap_or("").to_string();
            let timestamp = format_local_time(commit.time().seconds());

            entries.push(FileTimelineEntry {
                commit_hash: oid.to_string(),
                author: author_name,
                time: timestamp,
                message,
                diff: diff_text,
            });

            if entries.len() >= 50 {
                break;
            }
        }
    }

    Ok(entries)
}

#[derive(Serialize)]
struct HealthReport {
    large_files: Vec<String>,
    stale_branches: Vec<String>,
    conflicts: Vec<String>,
}

#[tauri::command]
async fn get_health_report(path: String) -> Result<HealthReport, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    get_health_report_impl(&repo)
}

// 无上游且 tip 提交超过该天数的分支才算废弃
const STALE_BRANCH_DAYS: i64 = 14;

fn get_health_report_impl(repo: &Repository) -> Result<HealthReport, String> {
    let mut large_files = Vec::new();
    let mut conflicts = Vec::new();

    if let Ok(statuses) = repo.statuses(None) {
        for entry in statuses.iter() {
            if entry.status() == git2::Status::CONFLICTED {
                if let Ok(p) = entry.path() {
                    conflicts.push(p.to_string());
                }
            }
        }
    }

    // 只读对象头取大小，find_blob 会把整个 blob 解压进内存
    if let Ok(tree) = repo.head().and_then(|h| h.peel_to_tree()) {
        if let Ok(odb) = repo.odb() {
            let _ = tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
                if entry.kind() == Some(git2::ObjectType::Blob) {
                    if let Ok((size, _)) = odb.read_header(entry.id()) {
                        if size > 1024 * 1024 {
                            if let Ok(name) = entry.name() {
                                large_files.push(format!("{}{}", root, name));
                            }
                        }
                    }
                }
                git2::TreeWalkResult::Ok
            });
        }
    }

    let mut stale_branches = Vec::new();
    let head_name = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().ok().map(|s| s.to_string()));
    let stale_cutoff = chrono::Utc::now().timestamp() - STALE_BRANCH_DAYS * 24 * 3600;
    if let Ok(branches) = repo.branches(Some(git2::BranchType::Local)) {
        for branch in branches {
            if let Ok((branch, _)) = branch {
                let name = branch.name().map_err(|e| format!("分支名错误: {}", e))?;
                let name = name.unwrap_or("未知").to_string();
                // 远程跟踪分支天然无 upstream；当前分支和活跃分支不算废弃
                if Some(&name) != head_name.as_ref()
                    && branch.upstream().is_err()
                    && branch
                        .get()
                        .peel_to_commit()
                        .map(|c| c.time().seconds() < stale_cutoff)
                        .unwrap_or(false)
                {
                    stale_branches.push(name);
                }
            }
        }
    }

    Ok(HealthReport {
        large_files,
        stale_branches,
        conflicts,
    })
}

#[derive(Serialize, Debug)]
struct CalendarDay {
    date: String,
    count: usize,
}

#[tauri::command]
async fn get_commit_calendar(path: String, days: Option<usize>) -> Result<Vec<CalendarDay>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    get_commit_calendar_impl(&repo, days.unwrap_or(365))
}

// 按本地日期聚合；时钟偏斜时跳过而非提前 break，处理量封顶
fn get_commit_calendar_impl(repo: &Repository, days: usize) -> Result<Vec<CalendarDay>, String> {
    let cutoff = chrono::Utc::now().timestamp() - days as i64 * 24 * 3600;
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut processed = 0usize;

    for oid in sorted_revwalk(repo)? {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let seconds = commit.time().seconds();
        if seconds < cutoff {
            continue;
        }
        let stamp = format_local_time(seconds);
        // "未知时间" 等多字节串按字节切会 panic，get 在非字符边界返回 None
        let date = stamp.get(..10).unwrap_or(stamp.as_str()).to_string();
        *counts.entry(date).or_insert(0) += 1;

        processed += 1;
        if processed >= 20000 {
            break;
        }
    }

    let mut result: Vec<CalendarDay> = counts
        .into_iter()
        .map(|(date, count)| CalendarDay { date, count })
        .collect();
    result.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(result)
}

#[derive(Serialize, Debug)]
struct DirectoryKnowledge {
    path: String,
    total_lines: usize,
    authors: usize,
    top_author: String,
    top_share: f64,
    bus_factor: usize,
}

// 逐提交统计各顶层目录的改动行数归属，近 1000 个提交
#[tauri::command]
async fn get_knowledge_report(path: String) -> Result<Vec<DirectoryKnowledge>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    get_knowledge_report_impl(&repo)
}

// 覆盖 50% 改动所需人数；1 表示单点依赖，根目录文件归入 "(root)"
fn get_knowledge_report_impl(repo: &Repository) -> Result<Vec<DirectoryKnowledge>, String> {
    let mut per_dir: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut processed = 0usize;

    for oid in sorted_revwalk(repo)? {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let author = commit.author().name().unwrap_or("未知").to_string();
        let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;
        let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());

        if let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            let deltas: Vec<git2::DiffDelta<'_>> = diff.deltas().collect();
            for (idx, delta) in deltas.iter().enumerate() {
                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path());
                let Some(path) = path else { continue };
                let path_str = path.to_string_lossy();
                let dir = match path_str.split_once('/') {
                    Some((head, _)) => head.to_string(),
                    None => "(root)".to_string(),
                };
                let (add, del) = git2::Patch::from_diff(&diff, idx)
                    .ok()
                    .flatten()
                    .map(|mut p| {
                        let mut add = 0;
                        let mut del = 0;
                        let _ = p.print(&mut |_delta, _hunk, line| {
                            match line.origin() {
                                '+' => add += 1,
                                '-' => del += 1,
                                _ => {}
                            }
                            true
                        });
                        (add, del)
                    })
                    .unwrap_or((0, 0));
                if add + del == 0 {
                    continue;
                }
                *per_dir
                    .entry(dir)
                    .or_default()
                    .entry(author.clone())
                    .or_insert(0) += add + del;
            }
        }

        processed += 1;
        if processed >= 1000 {
            break;
        }
    }

    let mut result = Vec::new();
    for (dir, authors) in per_dir {
        let total: usize = authors.values().sum();
        if total == 0 {
            continue;
        }
        let mut list: Vec<(String, usize)> = authors.into_iter().collect();
        list.sort_by(|a, b| b.1.cmp(&a.1));
        let (top_author, top_lines) = list[0].clone();
        let mut cumulative = 0usize;
        let mut bus_factor = 0usize;
        for (_, lines) in &list {
            cumulative += *lines;
            bus_factor += 1;
            if (cumulative as f64) / (total as f64) > 0.5 {
                break;
            }
        }
        result.push(DirectoryKnowledge {
            path: dir,
            total_lines: total,
            authors: list.len(),
            top_author,
            top_share: top_lines as f64 / total as f64,
            bus_factor,
        });
    }

    // 风险优先：巴士因子低、头部占比高、改动量大
    result.sort_by(|a, b| {
        a.bus_factor
            .cmp(&b.bus_factor)
            .then(
                b.top_share
                    .partial_cmp(&a.top_share)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(b.total_lines.cmp(&a.total_lines))
    });
    Ok(result)
}

#[derive(Serialize)]
struct Contributor {
    author: String,
    email: String,
    commits: usize,
    additions: usize,
    deletions: usize,
}

#[tauri::command]
async fn get_contributors(path: String) -> Result<Vec<Contributor>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    get_contributors_impl(&repo)
}

fn get_contributors_impl(repo: &Repository) -> Result<Vec<Contributor>, String> {
    let revwalk = sorted_revwalk(repo)?;

    let mut contributors: std::collections::HashMap<String, (String, usize, usize, usize)> = std::collections::HashMap::new();
    let mut processed = 0usize;

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let author = commit.author().name().unwrap_or("未知").to_string();
        let email = commit.author().email().unwrap_or("").to_string();

        let entry = contributors
            .entry(author)
            .or_insert((email, 0, 0, 0));
        entry.1 += 1;

        if let Ok(tree) = commit.tree() {
            let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
            if let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
                diff.foreach(
                    &mut |_delta, _| true,
                    None,
                    None,
                    Some(&mut |_delta, _hunk, line| {
                        match line.origin() {
                            '+' => entry.2 += 1,
                            '-' => entry.3 += 1,
                            _ => {}
                        }
                        true
                    }),
                )
                .map_err(|e| format!("Diff 统计失败: {}", e))?;
            }
        }

        // 上限按处理的提交数计，避免大仓库逐提交 diff
        processed += 1;
        if processed >= 500 {
            break;
        }
    }

    let mut result: Vec<Contributor> = contributors
        .into_iter()
        .map(|(author, (email, commits, additions, deletions))| Contributor {
            author,
            email,
            commits,
            additions,
            deletions,
        })
        .collect();
    result.sort_by(|a, b| b.commits.cmp(&a.commits));

    Ok(result)
}

#[derive(Serialize)]
struct HotFile {
    path: String,
    changes: usize,
}

#[tauri::command]
async fn get_hot_files(path: String) -> Result<Vec<HotFile>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    get_hot_files_impl(&repo)
}

fn get_hot_files_impl(repo: &Repository) -> Result<Vec<HotFile>, String> {
    let revwalk = sorted_revwalk(repo)?;

    let mut file_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut processed = 0;

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;

        if let Ok(tree) = commit.tree() {
            let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
            if let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
                diff.foreach(
                    &mut |delta, _| {
                        if let Some(path) = delta.new_file().path() {
                            let path_str = path.to_string_lossy().to_string();
                            *file_counts.entry(path_str).or_insert(0) += 1;
                        }
                        true
                    },
                    None,
                    None,
                    None,
                )
                .map_err(|e| format!("Diff 遍历失败: {}", e))?;
            }
        }

        processed += 1;
        if processed >= 500 {
            break;
        }
    }

    let mut result: Vec<HotFile> = file_counts
        .into_iter()
        .map(|(path, changes)| HotFile { path, changes })
        .collect();
    result.sort_by(|a, b| b.changes.cmp(&a.changes));
    result.truncate(20);

    Ok(result)
}

#[derive(Serialize)]
struct StashEntry {
    index: usize,
    message: String,
}

#[tauri::command]
fn stash_save(path: String, message: Option<String>) -> Result<String, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let mut repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let sig = repo.signature().map_err(|e| format!("无法获取签名: {}", e))?;

    let msg = message.unwrap_or_else(|| {
        chrono::Local::now().format("Stash %Y-%m-%d %H:%M:%S").to_string()
    });

    let oid = repo
        .stash_save(&sig, &msg, None)
        .map_err(|e| format!("Stash 保存失败: {}", e))?;

    Ok(oid.to_string())
}

#[tauri::command]
fn stash_list(path: String) -> Result<Vec<StashEntry>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let mut repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut entries = Vec::new();
    repo.stash_foreach(|index, message, _| {
        entries.push(StashEntry {
            index: index as usize,
            message: message.to_string(),
        });
        true
    })
    .map_err(|e| format!("Stash 遍历失败: {}", e))?;

    Ok(entries)
}

#[tauri::command]
fn stash_pop(path: String, index: usize) -> Result<(), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let mut repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    repo.stash_pop(index, None)
        .map_err(|e| format!("Stash pop 失败: {}", e))?;
    Ok(())
}

#[tauri::command]
fn stash_drop(path: String, index: usize) -> Result<(), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let mut repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    repo.stash_drop(index)
        .map_err(|e| format!("Stash drop 失败: {}", e))?;
    Ok(())
}

#[derive(Serialize)]
struct RebaseCommit {
    hash: String,
    message: String,
    author: String,
    time: String,
}

#[derive(Deserialize)]
struct RebaseOperation {
    hash: String,
    action: String,
    new_message: Option<String>,
}

#[tauri::command]
fn get_rebase_commits(path: String, count: usize) -> Result<Vec<RebaseCommit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    let commits = collect_commits(&repo, ANALYSIS_COMMIT_LIMIT)?;
    let result: Vec<RebaseCommit> = commits
        .into_iter()
        .take(count)
        .map(|c| RebaseCommit {
            hash: c.hash,
            message: c.message,
            author: c.author,
            time: c.time,
        })
        .collect();
    Ok(result)
}

#[tauri::command]
async fn execute_rebase(path: String, operations: Vec<RebaseOperation>) -> Result<String, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err("仓库存在未完成的操作（合并/拣选等），无法 rebase".into());
    }

    let head = repo.head().map_err(|e| format!("无法读取 HEAD: {}", e))?;
    if !head.is_branch() {
        return Err("当前处于游离 HEAD 状态，无法 rebase".into());
    }

    if operations.is_empty() {
        return Err("没有需要 rebase 的提交".into());
    }

    let statuses = repo
        .statuses(None)
        .map_err(|e| format!("无法读取仓库状态: {}", e))?;
    for entry in statuses.iter() {
        let dirty = entry.status().intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_RENAMED
                | git2::Status::WT_TYPECHANGE
                | git2::Status::CONFLICTED,
        );
        if dirty {
            return Err("工作区有未提交的更改，无法 rebase".into());
        }
    }

    // 定位窗口提交（newest-first）与基提交 HEAD~N
    let head_commit = head
        .peel_to_commit()
        .map_err(|e| format!("无法解析 HEAD: {}", e))?;
    let mut cur = head_commit.clone();
    let mut window: Vec<git2::Commit> = Vec::new();
    for _ in 0..operations.len() {
        window.push(cur.clone());
        cur = cur.parent(0).map_err(|_| {
            format!("提交数量不足，无法 rebase HEAD~{}", operations.len())
        })?;
    }
    let base = cur;

    for c in &window {
        if c.parent_count() > 1 {
            return Err("无法 rebase 包含合并提交的历史".into());
        }
    }

    let mut by_hash: HashMap<String, git2::Commit> = HashMap::new();
    for op in &operations {
        let oid = Oid::from_str(&op.hash)
            .map_err(|e| format!("无效的提交哈希 {}: {}", op.hash, e))?;
        let found = window
            .iter()
            .find(|c| c.id() == oid)
            .ok_or_else(|| format!("提交 {} 不在可 rebase 的范围内", op.hash))?;
        by_hash.insert(op.hash.clone(), found.clone());
    }

    let ordered: Vec<&RebaseOperation> = operations.iter().rev().collect();

    let committer = match repo.signature() {
        Ok(sig) => sig,
        Err(_) => {
            let a = head_commit.author();
            git2::Signature::new(
                a.name().unwrap_or("unknown"),
                a.email().unwrap_or("unknown"),
                &a.when(),
            )
            .map_err(|e| format!("无法确定提交身份: {}", e))?
        }
    };

    let mut current_oid = base.id();
    let mut current_tree = base
        .tree()
        .map_err(|e| format!("无法读取基提交树: {}", e))?;
    let mut pending: Vec<git2::Commit> = Vec::new();
    let mut pending_msgs: Vec<String> = Vec::new();

    for op in &ordered {
        let commit = by_hash.get(&op.hash).unwrap();
        match op.action.as_str() {
            "drop" => {
                rebase_flush(&repo, &mut current_oid, &mut current_tree, &mut pending, &mut pending_msgs, &committer)?;
            }
            "squash" => {
                if pending.is_empty() {
                    return Err("squash 前必须有一个 pick/reword 提交".into());
                }
                pending.push(commit.clone());
                pending_msgs.push(commit.message().unwrap_or("").to_string());
            }
            _ => {
                rebase_flush(&repo, &mut current_oid, &mut current_tree, &mut pending, &mut pending_msgs, &committer)?;
                let message = if op.action == "reword" {
                    op.new_message.clone().unwrap_or_else(|| {
                        commit.message().unwrap_or("").to_string()
                    })
                } else {
                    commit.message().unwrap_or("").to_string()
                };
                pending.push(commit.clone());
                pending_msgs.push(message);
            }
        }
    }
    rebase_flush(&repo, &mut current_oid, &mut current_tree, &mut pending, &mut pending_msgs, &committer)?;

    let final_commit = repo
        .find_commit(current_oid)
        .map_err(|e| format!("找不到最终提交: {}", e))?;
    repo.reset(&final_commit.as_object(), git2::ResetType::Hard, None)
        .map_err(|e| format!("更新工作区失败: {}", e))?;

    Ok("rebase 成功".into())
}

fn rebase_flush<'a>(
    repo: &'a Repository,
    current_oid: &mut Oid,
    current_tree: &mut git2::Tree<'a>,
    pending: &mut Vec<git2::Commit>,
    pending_msgs: &mut Vec<String>,
    committer: &git2::Signature,
) -> Result<(), String> {
    if pending.is_empty() {
        return Ok(());
    }

    // 将组内提交的改动依次 3-way merge 到当前树（等价于 git cherry-pick 的树级合并）
    let mut tree = current_tree.clone();
    for g in pending.iter() {
        let ancestor = g
            .parent(0)
            .map_err(|e| format!("无法读取父提交: {}", e))?
            .tree()
            .map_err(|e| format!("无法读取提交树: {}", e))?;
        let mut idx = repo
            .merge_trees(&ancestor, &tree, &g.tree().map_err(|e| format!("无法读取提交树: {}", e))?, None)
            .map_err(|e| format!("合并提交失败: {}", e))?;
        if idx.has_conflicts() {
            return Err("rebase 冲突，已回滚到原提交，请调整后重试".to_string());
        }
        let tree_oid = idx
            .write_tree_to(repo)
            .map_err(|e| format!("无法写入树: {}", e))?;
        tree = repo
            .find_tree(tree_oid)
            .map_err(|e| format!("找不到树: {}", e))?;
    }

    let message = pending_msgs.join("\n\n");
    let parent = repo
        .find_commit(*current_oid)
        .map_err(|e| format!("找不到父提交: {}", e))?;
    let author = {
        let a = pending[0].author();
        git2::Signature::new(
            a.name().unwrap_or("unknown"),
            a.email().unwrap_or("unknown"),
            &a.when(),
        )
        .map_err(|e| format!("创建签名失败: {}", e))?
    };
    let oid = repo
        .commit(None, &author, committer, &message, &tree, &[&parent])
        .map_err(|e| format!("创建提交失败: {}", e))?;
    *current_oid = oid;
    *current_tree = tree;
    pending.clear();
    pending_msgs.clear();
    Ok(())
}

#[derive(Serialize)]
struct SearchResult {
    commit_hash: String,
    author: String,
    time: String,
    file_path: String,
    line_number: usize,
    content: String,
}

#[tauri::command]
async fn semantic_search(path: String, query: String, all_refs: Option<bool>) -> Result<Vec<SearchResult>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    semantic_search_impl(&repo, &query, all_refs.unwrap_or(false))
}

fn semantic_search_impl(repo: &Repository, query: &str, all_refs: bool) -> Result<Vec<SearchResult>, String> {
    let revwalk = sorted_revwalk_refs(repo, all_refs)?;

    let mut results = Vec::new();
    let query_lower = query.to_lowercase();

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;

        let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .map_err(|e| format!("无法生成 Diff: {}", e))?;

        let deltas: Vec<git2::DiffDelta<'_>> = diff.deltas().collect();
        for (idx, delta) in deltas.iter().enumerate() {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let patch = git2::Patch::from_diff(&diff, idx)
                .map_err(|e| format!("Patch 创建失败: {}", e))?;
            if let Some(mut p) = patch {
                // 用 hunk 头的起始行号推算真实文件行号
                let mut old_ln = 0usize;
                let mut new_ln = 0usize;
                p.print(&mut |_delta, _hunk, line| {
                    match line.origin() {
                        'H' => {
                            let header = String::from_utf8_lossy(line.content());
                            let (os, _ol, ns, _nl) = parse_hunk_header(&header);
                            old_ln = os;
                            new_ln = ns;
                        }
                        'F' => {}
                        origin => {
                            let content = std::str::from_utf8(line.content()).unwrap_or("");
                            if content.to_lowercase().contains(&query_lower) {
                                let line_number = match origin {
                                    '-' => old_ln,
                                    _ => new_ln,
                                };
                                results.push(SearchResult {
                                    commit_hash: oid.to_string(),
                                    author: commit.author().name().unwrap_or("未知").to_string(),
                                    time: format_local_time(commit.time().seconds()),
                                    file_path: path.clone(),
                                    line_number,
                                    content: content.to_string(),
                                });
                            }
                            match origin {
                                '-' => old_ln += 1,
                                '+' => new_ln += 1,
                                ' ' => {
                                    old_ln += 1;
                                    new_ln += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                    true
                }).ok();
            }
        }

        if results.len() >= 200 {
            break;
        }
    }

    Ok(results)
}

#[derive(Serialize)]
struct DiffResult {
    commit_a: String,
    commit_b: String,
    diff: String,
}

#[tauri::command]
async fn compare_commits(path: String, commit_a: String, commit_b: String) -> Result<DiffResult, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let oid_a = Oid::from_str(&commit_a).map_err(|e| format!("无效的哈希 A: {}", e))?;
    let oid_b = Oid::from_str(&commit_b).map_err(|e| format!("无效的哈希 B: {}", e))?;

    let commit_a = repo.find_commit(oid_a).map_err(|e| format!("找不到提交 A: {}", e))?;
    let commit_b = repo.find_commit(oid_b).map_err(|e| format!("找不到提交 B: {}", e))?;

    let tree_a = commit_a.tree().map_err(|e| format!("无法获取树 A: {}", e))?;
    let tree_b = commit_b.tree().map_err(|e| format!("无法获取树 B: {}", e))?;

    let diff = repo
        .diff_tree_to_tree(Some(&tree_a), Some(&tree_b), None)
        .map_err(|e| format!("无法生成 Diff: {}", e))?;

    let mut diff_text = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        diff_text.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
        true
    }).map_err(|e| format!("Diff 打印失败: {}", e))?;

    Ok(DiffResult {
        commit_a: commit_a.id().to_string(),
        commit_b: commit_b.id().to_string(),
        diff: diff_text,
    })
}

#[derive(Serialize)]
struct ChangelogEntry {
    version: String,
    date: String,
    messages: Vec<String>,
}

#[tauri::command]
fn generate_changelog(path: String, count: usize) -> Result<Vec<ChangelogEntry>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    let commits = collect_commits(&repo, ANALYSIS_COMMIT_LIMIT)?;
    let mut entries: Vec<ChangelogEntry> = Vec::new();

    let mut current_date = String::new();
    let mut current_messages = Vec::new();

    for commit in commits.iter().take(count) {
        // "未知时间" 等多字节字符串按字节切片会 panic，get 在非字符边界返回 None
        let date = commit.time.get(..10).unwrap_or(commit.time.as_str());
        if date != current_date {
            if !current_date.is_empty() {
                entries.push(ChangelogEntry {
                    version: current_date.to_string(),
                    date: current_date.to_string(),
                    messages: current_messages.clone(),
                });
            }
            current_date = date.to_string();
            current_messages = Vec::new();
        }
        current_messages.push(commit.message.clone());
    }

    if !current_date.is_empty() {
        entries.push(ChangelogEntry {
            version: current_date.to_string(),
            date: current_date,
            messages: current_messages,
        });
    }

    Ok(entries)
}

#[derive(Serialize)]
struct GraphCommit {
    hash: String,
    author: String,
    time: String,
    message: String,
    parent_hashes: Vec<String>,
}

#[tauri::command]
async fn get_graph_commits(path: String) -> Result<Vec<GraphCommit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL).map_err(|e| format!("无法设置排序: {}", e))?;

    let mut results = Vec::new();
    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let timestamp = format_local_time(commit.time().seconds());

        let parents: Vec<String> = commit.parent_ids().map(|id| id.to_string()).collect();

        results.push(GraphCommit {
            hash: oid.to_string(),
            author: commit.author().name().unwrap_or("未知").to_string(),
            time: timestamp,
            message: commit.message().unwrap_or("").to_string(),
            parent_hashes: parents,
        });
        if results.len() >= 100 { break; }
    }
    Ok(results)
}

#[derive(Serialize)]
struct TreeNode {
    name: String,
    is_directory: bool,
    children: Vec<TreeNode>,
}

#[tauri::command]
async fn get_file_tree(path: String) -> Result<Vec<TreeNode>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let head = repo.head().map_err(|e| format!("无法获取 HEAD: {}", e))?;
    let commit = head.peel_to_commit().map_err(|e| format!("无法解引用: {}", e))?;
    let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;

    fn build_tree(tree: &git2::Tree<'_>, repo: &Repository, prefix: &str) -> Result<Vec<TreeNode>, String> {
        let mut nodes = std::collections::BTreeMap::new();
        for entry in tree.iter() {
            let name = if let Ok(n) = entry.name() {
                n.to_string()
            } else {
                continue;
            };
            let full_path = if prefix.is_empty() { name.clone() } else { format!("{}/{}", prefix, name) };
            if entry.kind() == Some(git2::ObjectType::Tree) {
                let sub_tree = repo.find_tree(entry.id()).map_err(|e| format!("{}", e))?;
                let children = build_tree(&sub_tree, repo, &full_path)?;
                nodes.insert(name.clone(), TreeNode { name, is_directory: true, children });
            } else {
                nodes.entry(name.clone()).or_insert(TreeNode { name, is_directory: false, children: vec![] });
            }
        }
        Ok(nodes.into_values().collect())
    }

    build_tree(&tree, &repo, "")
}

#[tauri::command]
async fn filter_commits(path: String, author: Option<String>, date_from: Option<String>, date_to: Option<String>, file_path: Option<String>, all_refs: Option<bool>) -> Result<Vec<Commit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    let all_refs = all_refs.unwrap_or(false);
    let all = collect_commits_refs(&repo, ANALYSIS_COMMIT_LIMIT, all_refs)?;
    let filtered: Vec<Commit> = all.into_iter().filter(|c| {
        if let Some(ref a) = author { if !c.author.to_lowercase().contains(&a.to_lowercase()) { return false; } }
        if let Some(ref df) = date_from { if c.time < *df { return false; } }
        if let Some(ref dt) = date_to {
            // 裸日期补到当天末尾比较，否则当天提交会被排除
            let bound = if dt.len() == 10 { format!("{} 23:59:59", dt) } else { dt.clone() };
            if c.time > bound { return false; }
        }
        true
    }).collect();

    // 文件路径筛选：先收集改动过该文件的提交集合，再交集过滤
    if let Some(ref fp) = file_path {
        if !fp.trim().is_empty() {
            let touching = commits_touching_file(&repo, fp.trim(), ANALYSIS_COMMIT_LIMIT, all_refs)?;
            return Ok(filtered.into_iter().filter(|c| touching.contains(&c.hash)).collect());
        }
    }
    Ok(filtered)
}

// 遍历历史，返回改动过指定文件的提交哈希集合（上限 limit 个提交）
fn commits_touching_file(repo: &Repository, file_path: &str, limit: usize, all_refs: bool) -> Result<std::collections::HashSet<String>, String> {
    let revwalk = sorted_revwalk_refs(repo, all_refs)?;

    let mut set = std::collections::HashSet::new();
    let mut processed = 0usize;
    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;
        let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
        if let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            for delta in diff.deltas() {
                let hit = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .map(|p| p.to_string_lossy() == file_path)
                    .unwrap_or(false);
                if hit {
                    set.insert(oid.to_string());
                    break;
                }
            }
        }
        processed += 1;
        if processed >= limit {
            break;
        }
    }
    Ok(set)
}

#[derive(Serialize)]
struct TagInfo {
    name: String,
    commit_hash: String,
}

#[tauri::command]
fn get_tags(path: String) -> Result<Vec<TagInfo>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut tags = Vec::new();
    for name in repo.tag_names(None).map_err(|e| format!("无法获取标签: {}", e))?.iter() {
        if let Ok(Some(name)) = name {
            if let Ok(obj) = repo.revparse_single(name) {
                tags.push(TagInfo { name: name.to_string(), commit_hash: obj.id().to_string() });
            }
        }
    }
    Ok(tags)
}

#[tauri::command]
fn create_tag(path: String, name: String, commit_hash: String) -> Result<(), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let oid = Oid::from_str(&commit_hash).map_err(|e| format!("无效哈希: {}", e))?;
    let obj = repo.find_object(oid, None).map_err(|e| format!("找不到对象: {}", e))?;
    repo.tag(&name, &obj, &repo.signature().map_err(|e| format!("签名失败: {}", e))?, "", false).map_err(|e| format!("创建标签失败: {}", e))?;
    Ok(())
}

#[derive(Serialize)]
struct RemoteInfo {
    name: String,
    url: String,
}

#[tauri::command]
fn get_remotes(path: String) -> Result<Vec<RemoteInfo>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut remotes = Vec::new();
    for name in repo.remotes().map_err(|e| format!("无法获取远程: {}", e))?.iter() {
        if let Ok(Some(name)) = name {
            if let Ok(url) = repo.find_remote(name) {
                remotes.push(RemoteInfo { name: name.to_string(), url: url.url().unwrap_or("").to_string() });
            }
        }
    }
    Ok(remotes)
}

#[derive(Serialize, Clone, Debug)]
struct InlineChange {
    offset: usize,
    length: usize,
    kind: String,
}
#[derive(Serialize, Clone, Debug)]
struct DiffLine {
    origin: String,
    content: String,
    inline_changes: Vec<InlineChange>,
}

#[derive(Serialize)]
struct DiffHunk {
    old_start: usize,
    old_lines: usize,
    new_start: usize,
    new_lines: usize,
    lines: Vec<DiffLine>,
}

#[derive(Serialize)]
struct DiffDetail {
    old_content: String,
    new_content: String,
    hunks: Vec<DiffHunk>,
}
// 返回 (删除段, 插入段)，偏移按字符计，供前端行内高亮
fn paired_inline_changes(old_line: &str, new_line: &str) -> (Vec<InlineChange>, Vec<InlineChange>) {
    use similar::TextDiff;
    let mut deletes = Vec::new();
    let mut inserts = Vec::new();
    let diff = TextDiff::from_words(old_line, new_line);
    let mut old_off = 0usize;
    let mut new_off = 0usize;
    for change in diff.iter_all_changes() {
        let chars = change.as_str().unwrap_or("").chars().count();
        match change.tag() {
            similar::ChangeTag::Equal => {
                old_off += chars;
                new_off += chars;
            }
            similar::ChangeTag::Delete => {
                deletes.push(InlineChange {
                    offset: old_off,
                    length: chars,
                    kind: "delete".to_string(),
                });
                old_off += chars;
            }
            similar::ChangeTag::Insert => {
                inserts.push(InlineChange {
                    offset: new_off,
                    length: chars,
                    kind: "insert".to_string(),
                });
                new_off += chars;
            }
        }
    }
    (deletes, inserts)
}

fn whole_line_changes(char_len: usize, kind: &str) -> Vec<InlineChange> {
    if char_len == 0 {
        vec![]
    } else {
        vec![InlineChange {
            offset: 0,
            length: char_len,
            kind: kind.to_string(),
        }]
    }
}

// 缓冲的 -/+ 行按序两两配对，未配对的整行高亮
fn flush_paired_lines(
    current_lines: &mut Vec<DiffLine>,
    minus_buf: &mut Vec<String>,
    plus_buf: &mut Vec<String>,
) {
    for (i, m) in minus_buf.iter().enumerate() {
        let inline_changes = match plus_buf.get(i) {
            Some(p) => paired_inline_changes(m, p).0,
            None => whole_line_changes(m.chars().count(), "delete"),
        };
        current_lines.push(DiffLine {
            origin: "-".to_string(),
            content: m.clone(),
            inline_changes,
        });
    }
    for (i, p) in plus_buf.iter().enumerate() {
        let inline_changes = match minus_buf.get(i) {
            Some(m) => paired_inline_changes(m, p).1,
            None => whole_line_changes(p.chars().count(), "insert"),
        };
        current_lines.push(DiffLine {
            origin: "+".to_string(),
            content: p.clone(),
            inline_changes,
        });
    }
    minus_buf.clear();
    plus_buf.clear();
}

#[tauri::command]
fn get_diff_detail(path: String, commit_hash: String) -> Result<Vec<(String, DiffDetail)>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let oid = Oid::from_str(&commit_hash).map_err(|e| format!("无效哈希: {}", e))?;
    let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
    let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;
    let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None).map_err(|e| format!("无法生成 Diff: {}", e))?;
    let deltas: Vec<git2::DiffDelta<'_>> = diff.deltas().collect();

    let mut results = Vec::new();

    for (idx, delta) in deltas.iter().enumerate() {
        // 删除的文件回退到 old_file
        let new_path = delta.new_file().path();
        let old_path = delta.old_file().path();
        let file_path = new_path
            .or(old_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let old_content = match (parent_tree.as_ref(), old_path) {
            (Some(pt), Some(op)) => pt.get_path(op).ok().and_then(|e| repo.find_blob(e.id()).ok()).map(|b| String::from_utf8_lossy(b.content()).to_string()).unwrap_or_default(),
            _ => String::new(),
        };
        let new_content = new_path
            .and_then(|np| tree.get_path(np).ok())
            .and_then(|e| repo.find_blob(e.id()).ok())
            .map(|b| String::from_utf8_lossy(b.content()).to_string())
            .unwrap_or_default();

        let patch = git2::Patch::from_diff(&diff, idx).map_err(|e| format!("Patch 创建失败: {}", e))?;
        let mut hunks = Vec::new();
        if let Some(mut p) = patch {
            let mut current_old_start = 0usize;
            let mut current_new_start = 0usize;
            let mut current_lines: Vec<DiffLine> = Vec::new();
            let mut minus_buf: Vec<String> = Vec::new();
            let mut plus_buf: Vec<String> = Vec::new();

            p.print(&mut |_delta, _hunk, line| {
                match line.origin() {
                    // 'F' = 文件头行，'H' = hunk 头行（@@ -a,b +c,d @@）
                    'F' => {}
                    'H' => {
                        flush_paired_lines(&mut current_lines, &mut minus_buf, &mut plus_buf);
                        flush_hunk(&mut hunks, &mut current_lines, current_old_start, current_new_start);
                        let header = String::from_utf8_lossy(line.content());
                        let (os, _ol, ns, _nl) = parse_hunk_header(&header);
                        current_old_start = os;
                        current_new_start = ns;
                    }
                    '+' => {
                        plus_buf.push(String::from_utf8_lossy(line.content()).to_string());
                    }
                    // 只在上下文/hunk 头/结尾结算；提前 flush 会破坏连续删除行的配对
                    '-' => {
                        minus_buf.push(String::from_utf8_lossy(line.content()).to_string());
                    }
                    origin => {
                        flush_paired_lines(&mut current_lines, &mut minus_buf, &mut plus_buf);
                        let origin_str = match origin {
                            ' ' => " ",
                            _ => "?",
                        };
                        current_lines.push(DiffLine {
                            origin: origin_str.to_string(),
                            content: String::from_utf8_lossy(line.content()).to_string(),
                            inline_changes: vec![],
                        });
                    }
                }
                true
            }).map_err(|e| format!("Diff 打印失败: {}", e))?;
            flush_paired_lines(&mut current_lines, &mut minus_buf, &mut plus_buf);
            flush_hunk(&mut hunks, &mut current_lines, current_old_start, current_new_start);
        }
        results.push((file_path, DiffDetail { old_content, new_content, hunks }));
    }
    Ok(results)
}

// 收尾当前 hunk，行数按 origin 统计
fn flush_hunk(hunks: &mut Vec<DiffHunk>, lines: &mut Vec<DiffLine>, old_start: usize, new_start: usize) {
    if lines.is_empty() {
        return;
    }
    let old_lines = lines.iter().filter(|l| l.origin == "-" || l.origin == " ").count();
    let new_lines = lines.iter().filter(|l| l.origin == "+" || l.origin == " ").count();
    hunks.push(DiffHunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines: std::mem::take(lines),
    });
}

fn parse_hunk_header(header: &str) -> (usize, usize, usize, usize) {
    // 头格式: "@@ -old_start,old_lines +new_start,new_lines @@ 上下文"
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() < 4 {
        return (0, 0, 0, 0);
    }
    let parse_range = |s: &str, sign: char| -> (usize, usize) {
        let s = s.trim_start_matches(sign);
        match s.split_once(',') {
            Some((start, count)) => (
                start.parse::<isize>().unwrap_or(0).unsigned_abs(),
                count.parse::<usize>().unwrap_or(1),
            ),
            None => (s.parse::<isize>().unwrap_or(0).unsigned_abs(), 1),
        }
    };
    // parts[0] 是 "@@"，区间在 parts[1]（旧）与 parts[2]（新）
    let (old_start, old_lines) = parse_range(parts[1], '-');
    let (new_start, new_lines) = parse_range(parts[2], '+');
    (old_start, old_lines, new_start, new_lines)
}

#[tauri::command]
async fn get_commits_paginated(path: String, page: usize, page_size: usize) -> Result<(Vec<Commit>, usize), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    // 单遍遍历：跳过前 start 条取一页，顺便统计总数
    let start = page.saturating_mul(page_size);
    let end = start.saturating_add(page_size);
    let mut total = 0usize;
    let mut page_data = Vec::new();
    for oid in sorted_revwalk(&repo)? {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        if total >= start && total < end {
            let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
            page_data.push(commit_to_json(oid, &commit));
        }
        total += 1;
    }
    Ok((page_data, total))
}

#[tauri::command]
fn get_hooks(path: String) -> Result<Vec<String>, String> {
    let hooks_dir = format!("{}/.git/hooks", shellexpand::tilde(&path));
    let mut hooks = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&hooks_dir) {
        for entry in entries.flatten() {
            // 只列文件，hooks 目录下的子目录不是可执行 hook
            if !entry.path().is_file() { continue; }
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".sample") { hooks.push(name); }
        }
    }
    Ok(hooks)
}

#[tauri::command]
fn get_hook_content(path: String, hook_name: String) -> Result<String, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let hooks_dir = Path::new(&expanded).join(".git").join("hooks");
    let relative = safe_relative_path(&hook_name)?;
    let hook_path = resolve_existing_under(&hooks_dir, &relative)?;
    std::fs::read_to_string(&hook_path).map_err(|e| format!("读取失败: {}", e))
}

#[tauri::command]
fn save_hook_content(path: String, hook_name: String, content: String) -> Result<(), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let hooks_dir = Path::new(&expanded).join(".git").join("hooks");
    let relative = safe_relative_path(&hook_name)?;
    let hooks_canonical = hooks_dir
        .canonicalize()
        .map_err(|e| format!("hooks 目录不可用: {}", e))?;
    let hook_path = hooks_canonical.join(&relative);
    // hooks 目录是平铺的：多级路径的中间目录必须已存在，否则拒绝
    if let Some(parent) = hook_path.parent() {
        if !parent.is_dir() {
            return Err("hook 路径无效".into());
        }
    }
    // 已存在的同名文件若是指向目录外的符号链接，拒绝写入
    if hook_path.exists() {
        let canonical = hook_path
            .canonicalize()
            .map_err(|e| format!("路径无效: {}", e))?;
        if !canonical.starts_with(&hooks_canonical) {
            return Err("拒绝写入 hooks 目录之外的文件".into());
        }
    }
    std::fs::write(&hook_path, content).map_err(|e| format!("写入失败: {}", e))?;
    // 新建的 hook 需要 +x 才会被 git 执行
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)
            .map_err(|e| format!("读取权限失败: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)
            .map_err(|e| format!("设置执行权限失败: {}", e))?;
    }
    Ok(())
}

#[derive(Serialize)]
struct ConflictDetail {
    path: String,
    ours: String,
    theirs: String,
    merged: String,
    conflict_blocks: Vec<ConflictBlock>,
}

#[derive(Serialize)]
struct ConflictBlock {
    ours_text: String,
    theirs_text: String,
}

enum ConflictParseState {
    Normal,
    Ours,
    Base,
    Theirs,
}

struct ParsedConflicts {
    blocks: Vec<ConflictBlock>,
    // 冲突块之间的普通文本，长度恒为 blocks.len() + 1
    merged_parts: Vec<String>,
}

fn parse_conflict_content(content: &str) -> ParsedConflicts {
    let mut blocks = Vec::new();
    let mut merged_parts = Vec::new();
    let mut current_part = String::new();
    let mut ours_lines: Vec<String> = Vec::new();
    let mut theirs_lines: Vec<String> = Vec::new();
    let mut state = ConflictParseState::Normal;

    for line in content.lines() {
        match state {
            ConflictParseState::Normal => {
                // 只在非冲突状态下把 <<<<<<< 当作冲突开始，避免嵌套/孤立标记误判
                if line.starts_with("<<<<<<<") {
                    state = ConflictParseState::Ours;
                } else {
                    current_part.push_str(line);
                    current_part.push('\n');
                }
            }
            ConflictParseState::Ours => {
                if line.starts_with("|||||||") {
                    // diff3 风格的公共祖先段，既不属于 ours 也不属于 theirs
                    state = ConflictParseState::Base;
                } else if line.starts_with("=======") {
                    state = ConflictParseState::Theirs;
                } else {
                    ours_lines.push(line.to_string());
                }
            }
            ConflictParseState::Base => {
                if line.starts_with("=======") {
                    state = ConflictParseState::Theirs;
                }
            }
            ConflictParseState::Theirs => {
                if line.starts_with(">>>>>>>") {
                    blocks.push(ConflictBlock {
                        ours_text: ours_lines.join("\n"),
                        theirs_text: theirs_lines.join("\n"),
                    });
                    ours_lines.clear();
                    theirs_lines.clear();
                    merged_parts.push(std::mem::take(&mut current_part));
                    state = ConflictParseState::Normal;
                } else {
                    theirs_lines.push(line.to_string());
                }
            }
        }
    }

    merged_parts.push(current_part);
    ParsedConflicts { blocks, merged_parts }
}

// 解决文本插回原位；空文本回退 ours
fn build_resolved_content(parsed: &ParsedConflicts, resolutions: &[String]) -> String {
    let mut out = String::new();
    for (i, block) in parsed.blocks.iter().enumerate() {
        out.push_str(&parsed.merged_parts[i]);
        let chosen = if resolutions.get(i).map_or(true, |r| r.is_empty()) {
            block.ours_text.as_str()
        } else {
            resolutions[i].as_str()
        };
        if !chosen.is_empty() {
            out.push_str(chosen);
            out.push('\n');
        }
    }
    out.push_str(parsed.merged_parts.last().map(String::as_str).unwrap_or(""));
    out
}

#[tauri::command]
fn get_conflict_detail(path: String) -> Result<Vec<ConflictDetail>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut conflict_files = Vec::new();

    if let Ok(statuses) = repo.statuses(None) {
        for entry in statuses.iter() {
            if entry.status() == git2::Status::CONFLICTED {
                let file_path = entry.path().unwrap_or("未知").to_string();
                let full_path = Path::new(&expanded).join(&file_path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let parsed = parse_conflict_content(&content);
                    let ours = parsed.blocks.iter().map(|b| b.ours_text.as_str()).collect::<Vec<&str>>().join("\n");
                    let theirs = parsed.blocks.iter().map(|b| b.theirs_text.as_str()).collect::<Vec<&str>>().join("\n");
                    let merged = parsed.merged_parts.concat();

                    conflict_files.push(ConflictDetail {
                        path: file_path,
                        ours,
                        theirs,
                        merged,
                        conflict_blocks: parsed.blocks,
                    });
                }
            }
        }
    }
    Ok(conflict_files)
}

#[tauri::command]
fn resolve_conflict(path: String, file_path: String, resolutions: Vec<String>) -> Result<(), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let relative = safe_relative_path(&file_path)?;
    let full_path = resolve_existing_under(Path::new(&expanded), &relative)?;
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("读取失败: {}", e))?;
    let parsed = parse_conflict_content(&content);
    if parsed.blocks.len() != resolutions.len() {
        return Err(format!(
            "文件冲突块数量已变化（当前 {} 个），请重新加载后再试",
            parsed.blocks.len()
        ));
    }
    let resolved = build_resolved_content(&parsed, &resolutions);
    std::fs::write(&full_path, resolved).map_err(|e| format!("写入失败: {}", e))?;

    // 写入索引（stage 0），真正把该文件标记为已解决
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut index = repo.index().map_err(|e| format!("无法读取索引: {}", e))?;
    index.add_path(Path::new(&file_path)).map_err(|e| format!("标记已解决失败: {}", e))?;
    index.write().map_err(|e| format!("写入索引失败: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn export_report_markdown(path: String) -> Result<String, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let health = get_health_report_impl(&repo)?;
    let contributors = get_contributors_impl(&repo)?;
    let hot_files = get_hot_files_impl(&repo)?;

    let mut md = String::from("# 仓库分析报告\n\n## 健康报告\n");
    md.push_str(&format!("- 大文件: {}\n", health.large_files.join(", ")));
    md.push_str(&format!("- 无上游分支: {}\n", health.stale_branches.join(", ")));
    md.push_str(&format!("- 冲突文件: {}\n\n", health.conflicts.join(", ")));
    md.push_str("## 贡献者统计\n");
    for c in &contributors {
        md.push_str(&format!("- {}: {} 次提交, +{}/-{}\n", c.author, c.commits, c.additions, c.deletions));
    }
    md.push_str("\n## 热点文件\n");
    for f in &hot_files {
        md.push_str(&format!("- {}: {} 次变更\n", f.path, f.changes));
    }
    Ok(md)
}

#[tauri::command]
fn list_scripts() -> Result<Vec<String>, String> {
    let scripts_dir = shellexpand::tilde("~/.git-tool/scripts").to_string();
    let dir = Path::new(&scripts_dir);
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建脚本目录失败: {}", e))?;
    }
    let mut scripts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    scripts.push(name.to_string_lossy().to_string());
                }
            }
        }
    }
    Ok(scripts)
}

fn run_script_impl(path: &str, script_name: &str) -> Result<String, String> {
    let expanded = shellexpand::tilde(path).to_string();
    let scripts_dir = shellexpand::tilde("~/.git-tool/scripts").to_string();
    let relative = safe_relative_path(script_name)?;
    let script_path = resolve_existing_under(Path::new(&scripts_dir), &relative)?;

    let output = std::process::Command::new(&script_path)
        .arg(&expanded)
        .output()
        .map_err(|e| format!("执行脚本失败: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// 进程等待会阻塞线程，放 spawn_blocking
#[tauri::command]
async fn run_script(path: String, script_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || run_script_impl(&path, &script_name))
        .await
        .map_err(|e| format!("脚本任务调度失败: {}", e))?
}

// SQL 查询引擎

#[derive(Serialize, Debug)]
struct QueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    elapsed_ms: u64,
    // 命中行数上限时为 true：截断必须让前端能说明，否则用户以为这就是全部结果
    truncated: bool,
}

#[derive(Clone)]
struct FileChangeRow {
    commit_hash: String,
    file_path: String,
    status: String,
    additions: usize,
    deletions: usize,
}

// 收集某个仓库的所有提交和文件变更记录
fn collect_all_data(repo: &Repository) -> Result<(Vec<Commit>, Vec<FileChangeRow>), String> {
    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL).map_err(|e| format!("无法设置排序: {}", e))?;

    let mut commits = Vec::new();
    let mut files = Vec::new();

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;
        let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());

        let time = commit.time();
        let timestamp = format_local_time(time.seconds());

        let hash = oid.to_string();
        commits.push(Commit {
            hash: hash.clone(),
            author: commit.author().name().unwrap_or("未知").to_string(),
            time: timestamp,
            message: commit.message().unwrap_or("").to_string(),
        });

        if let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            let deltas: Vec<git2::DiffDelta<'_>> = diff.deltas().collect();
            for (idx, delta) in deltas.iter().enumerate() {
                let status = match delta.status() {
                    git2::Delta::Added => "A",
                    git2::Delta::Deleted => "D",
                    git2::Delta::Modified => "M",
                    git2::Delta::Renamed => "R",
                    _ => "?",
                };
                let file_path = delta.new_file().path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                // 按文件各自的 patch 统计增删行，避免把整笔提交的总量算到每个文件上
                let (additions, deletions) = git2::Patch::from_diff(&diff, idx)
                    .ok()
                    .flatten()
                    .map(|mut p| {
                        let mut add = 0;
                        let mut del = 0;
                        let _ = p.print(&mut |_delta, _hunk, line| {
                            match line.origin() {
                                '+' => add += 1,
                                '-' => del += 1,
                                _ => {}
                            }
                            true
                        });
                        (add, del)
                    })
                    .unwrap_or((0, 0));

                files.push(FileChangeRow {
                    commit_hash: hash.clone(),
                    file_path,
                    status: status.to_string(),
                    additions,
                    deletions,
                });
            }
        }

        if commits.len() >= QUERY_COMMIT_LIMIT { break; }
    }

    Ok((commits, files))
}

// CONTAINS 'x' 改写为 LIKE '%x%'（转义 % _ \），字符串字面量内不改写
fn rewrite_contains(sql: &str) -> String {
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            // 原样复制整个字符串字面量（含 '' 转义）
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        out.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if is_word_char(c) {
            let start = i;
            while i < chars.len() && is_word_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if word.eq_ignore_ascii_case("contains") {
                let mut j = i;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if chars.get(j) == Some(&'\'') {
                    let mut k = j + 1;
                    let mut val = String::new();
                    // SQL 字面量的 '' 转义：单引号要还原进值里，否则 a''b 产出坏 SQL
                    while k < chars.len() {
                        if chars[k] == '\'' {
                            if chars.get(k + 1) == Some(&'\'') {
                                val.push('\'');
                                k += 2;
                                continue;
                            }
                            break;
                        }
                        val.push(chars[k]);
                        k += 1;
                    }
                    if k < chars.len() {
                        let escaped = val
                            .replace('\\', "\\\\")
                            .replace('%', "\\%")
                            .replace('_', "\\_")
                            .replace('\'', "''");
                        // 不带前导空格：关键词前的空白已原样保留
                        out.push_str("LIKE '%");
                        out.push_str(&escaped);
                        out.push_str("%' ESCAPE '\\'");
                        i = k + 1;
                        continue;
                    }
                }
            }
            out.push_str(&word);
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn format_sql_error(e: &rusqlite::Error, timeout: std::time::Duration) -> String {
    if let rusqlite::Error::SqliteFailure(err, _) = e {
        if err.code == rusqlite::ErrorCode::OperationInterrupted {
            return format!(
                "查询超过 {} 秒已被中止。多表连接或递归查询可能产生了爆炸式行数，请加上 WHERE / LIMIT 缩小范围",
                timeout.as_secs()
            );
        }
    }
    let msg = e.to_string();
    if msg.contains("too big") || msg.contains("exceeds size limit") {
        return format!("{}（单个字符串/BLOB 上限 {} MB）", msg, QUERY_VALUE_LIMIT / 1024 / 1024);
    }
    if msg.contains("no such table") {
        format!("表不存在，可用表: commits, file_changes（{}）", msg)
    } else {
        msg
    }
}

// 查询超时；失控的递归 CTE 只能靠它中止
const QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
// 返回行数上限；自连接的行数是乘积
const QUERY_ROW_LIMIT: usize = 5000;
// 单值大小上限；hex(zeroblob(1e8)) 能一次产出几百 MB
const QUERY_VALUE_LIMIT: i32 = 16 * 1024 * 1024;

#[tauri::command]
async fn git_query(path: String, sql: String) -> Result<QueryResult, String> {
    git_query_impl(&path, &sql, QUERY_TIMEOUT, QUERY_ROW_LIMIT)
}

fn git_query_impl(
    path: &str,
    sql: &str,
    timeout: std::time::Duration,
    row_limit: usize,
) -> Result<QueryResult, String> {
    let start = std::time::Instant::now();

    let expanded = shellexpand::tilde(path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let (commits, file_changes) = collect_all_data(&repo)?;

    let sql = rewrite_contains(sql);

    let mut conn = rusqlite::Connection::open_in_memory()
        .map_err(|e| format!("无法创建查询引擎: {}", e))?;
    conn.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        QUERY_VALUE_LIMIT,
    )
    .map_err(|e| format!("无法设置取值上限: {}", e))?;
    conn.execute_batch(
        "CREATE TABLE commits (
            hash TEXT COLLATE NOCASE,
            author TEXT COLLATE NOCASE,
            time TEXT COLLATE NOCASE,
            message TEXT COLLATE NOCASE
        );
        CREATE TABLE file_changes (
            commit_hash TEXT COLLATE NOCASE,
            file_path TEXT COLLATE NOCASE,
            status TEXT COLLATE NOCASE,
            additions INTEGER,
            deletions INTEGER
        );",
    )
    .map_err(|e| format!("初始化表失败: {}", e))?;

    {
        let mut stmt = conn
            .prepare("INSERT INTO commits VALUES (?1, ?2, ?3, ?4)")
            .map_err(|e| format!("初始化表失败: {}", e))?;
        for c in &commits {
            stmt.execute(rusqlite::params![c.hash, c.author, c.time, c.message])
                .map_err(|e| format!("写入数据失败: {}", e))?;
        }
    }
    {
        let mut stmt = conn
            .prepare("INSERT INTO file_changes VALUES (?1, ?2, ?3, ?4, ?5)")
            .map_err(|e| format!("初始化表失败: {}", e))?;
        for f in &file_changes {
            stmt.execute(rusqlite::params![f.commit_hash, f.file_path, f.status, f.additions as i64, f.deletions as i64])
                .map_err(|e| format!("写入数据失败: {}", e))?;
        }
    }

    // 查询阶段只读：拒绝 ATTACH、PRAGMA、INSERT/UPDATE/DELETE 等非查询操作
    conn.authorizer(Some(|ctx: rusqlite::hooks::AuthContext| {
        match ctx.action {
            rusqlite::hooks::AuthAction::Select
            | rusqlite::hooks::AuthAction::Read { .. }
            | rusqlite::hooks::AuthAction::Function { .. } => {
                rusqlite::hooks::Authorization::Allow
            }
            _ => rusqlite::hooks::Authorization::Deny,
        }
    }))
    .map_err(|e| format!("无法设置查询权限: {}", e))?;

    // progress 回调可中止「不出结果行但一直在算」的查询
    let deadline = std::time::Instant::now() + timeout;
    conn.progress_handler(100_000, Some(move || std::time::Instant::now() > deadline))
        .map_err(|e| format!("无法设置查询超时: {}", e))?;

    let mut stmt = conn.prepare(&sql).map_err(|e| format_sql_error(&e, timeout))?;
    let columns: Vec<String> = stmt.column_names().map(|s| s.to_string()).collect();
    let mut rows_iter = stmt.query(rusqlite::params![]).map_err(|e| format_sql_error(&e, timeout))?;
    let mut result_rows: Vec<Vec<String>> = Vec::new();
    let mut truncated = false;
    while let Some(row) = rows_iter.next().map_err(|e| format_sql_error(&e, timeout))? {
        // 到上限就停止拉取：SQLite 是流式出行，连接这类查询不会先把乘积全算出来
        if result_rows.len() >= row_limit {
            truncated = true;
            break;
        }
        let mut row_out = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            let cell = match row.get_ref(i).map_err(|e| format_sql_error(&e, timeout))? {
                rusqlite::types::ValueRef::Null => String::new(),
                rusqlite::types::ValueRef::Integer(n) => n.to_string(),
                rusqlite::types::ValueRef::Real(f) => format!("{}", f),
                rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
                rusqlite::types::ValueRef::Blob(b) => String::from_utf8_lossy(b).to_string(),
            };
            row_out.push(cell);
        }
        result_rows.push(row_out);
    }

    let elapsed = start.elapsed().as_millis() as u64;
    Ok(QueryResult {
        columns,
        rows: result_rows,
        elapsed_ms: elapsed,
        truncated,
    })
}

// Git 时间机器

#[derive(Serialize)]
struct TimeMachineSnapshot {
    commit_hash: String,
    author: String,
    time: String,
    message: String,
    files: Vec<TimeMachineFile>,
}

#[derive(Serialize)]
struct TimeMachineFile {
    path: String,
    size: usize,
    is_directory: bool,
}

#[tauri::command]
fn get_file_content_at_commit(path: String, commit_hash: String, file_path: String) -> Result<String, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;
    let oid = Oid::from_str(&commit_hash).map_err(|e| format!("无效的哈希: {}", e))?;
    let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
    let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;

    let entry = tree.get_path(Path::new(&file_path))
        .map_err(|e| format!("找不到文件: {}", e))?;
    let blob = repo.find_blob(entry.id()).map_err(|e| format!("无法获取文件内容: {}", e))?;
    Ok(String::from_utf8_lossy(blob.content()).to_string())
}

#[tauri::command]
fn get_file_content(path: String, file_path: String) -> Result<String, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let relative = safe_relative_path(&file_path)?;
    let full_path = resolve_existing_under(Path::new(&expanded), &relative)?;
    let content = std::fs::read(full_path).map_err(|e| format!("无法读取文件: {}", e))?;
    Ok(String::from_utf8_lossy(&content).to_string())
}

#[tauri::command]
async fn get_time_machine_snapshot(path: String, timestamp: i64) -> Result<TimeMachineSnapshot, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL).map_err(|e| format!("无法设置排序: {}", e))?;

    let mut closest_commit: Option<(Oid, i64)> = None;
    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let time = commit.time().seconds();
        if time <= timestamp {
            closest_commit = Some((oid, time));
            break;
        }
    }

    let (oid, _) = closest_commit.ok_or("没有找到该时间点之前的提交")?;
    let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
    let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;

    let commit_hash = oid.to_string();
    let author_name = commit.author().name().unwrap_or("未知").to_string();
    let timestamp_str = format_local_time(commit.time().seconds());
    let message = commit.message().unwrap_or("").to_string();

    let mut files = Vec::new();
    // 只读对象头取大小，拖滑块时不逐个解压 blob
    let odb = repo.odb().map_err(|e| format!("无法打开对象库: {}", e))?;
    tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        let name = entry.name().unwrap_or("未知");
        // walk 回调的 root 是带尾斜杠的父目录前缀（顶层为空串），直接拼接即完整路径
        let full_path = format!("{}{}", root, name);
        let kind = entry.kind();
        let is_dir = kind == Some(git2::ObjectType::Tree);
        let size = if is_dir {
            0
        } else {
            odb.read_header(entry.id()).map(|(size, _)| size).unwrap_or(0)
        };
        files.push(TimeMachineFile {
            path: full_path,
            size,
            is_directory: is_dir,
        });
        git2::TreeWalkResult::Ok
    }).map_err(|e| format!("遍历树失败: {}", e))?;

    Ok(TimeMachineSnapshot {
        commit_hash,
        author: author_name,
        time: timestamp_str,
        message,
        files,
    })
}

#[tauri::command]
async fn pick_background_image(app: tauri::AppHandle) -> Result<String, String> {
    // blocking_pick_file 会阻塞线程，放到线程池
    let file_path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| format!("文件对话框失败: {}", e))?;
    match file_path {
        Some(path) => {
            let p = path.as_path().ok_or("路径无效")?;
            let data = std::fs::read(p).map_err(|e| format!("读取文件失败: {}", e))?;
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            Ok(STANDARD.encode(&data))
        }
        None => Err("用户取消了选择".into()),
    }
}

#[tauri::command]
async fn open_folder_dialog(app: tauri::AppHandle) -> Result<String, String> {
    let folder_path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().blocking_pick_folder()
    })
    .await
    .map_err(|e| format!("文件夹对话框失败: {}", e))?;
    match folder_path {
        Some(path) => {
            let p = path.as_path().ok_or("路径无效")?;
            Ok(p.to_string_lossy().to_string())
        }
        None => Err("用户取消了选择".into()),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_commits,
            get_branches,
            checkout_branch,
            get_commit_detail,
            search_commits,
            get_blame,
            get_file_timeline,
            get_health_report,
            get_contributors,
            get_hot_files,
            stash_save,
            stash_list,
            stash_pop,
            stash_drop,
            get_rebase_commits,
            execute_rebase,
            semantic_search,
            compare_commits,
            generate_changelog,
            get_graph_commits,
            get_file_tree,
            filter_commits,
            get_tags,
            create_tag,
            get_remotes,
            get_diff_detail,
            get_commits_paginated,
            get_hooks,
            get_hook_content,
            save_hook_content,
            get_conflict_detail,
            resolve_conflict,
            export_report_markdown,
            list_scripts,
            run_script,
            get_time_machine_snapshot,
            get_file_content_at_commit,
            get_file_content,
            pick_background_image,
            open_folder_dialog,
            git_query,
            get_commit_calendar,
            get_knowledge_report
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}

#[cfg(test)]
mod conflict_tests {
    use super::*;

    #[test]
    fn parses_simple_conflict() {
        let content = "line1\n<<<<<<< HEAD\nours line\n=======\ntheirs line\n>>>>>>> feature\nline2\n";
        let parsed = parse_conflict_content(content);
        assert_eq!(parsed.blocks.len(), 1);
        assert_eq!(parsed.blocks[0].ours_text, "ours line");
        assert_eq!(parsed.blocks[0].theirs_text, "theirs line");
        assert_eq!(
            parsed.merged_parts,
            vec!["line1\n".to_string(), "line2\n".to_string()]
        );
    }

    #[test]
    fn rebuild_inserts_resolution_at_original_position() {
        let content = "line1\n<<<<<<< HEAD\nours line\n=======\ntheirs line\n>>>>>>> feature\nline2\n";
        let parsed = parse_conflict_content(content);
        let resolved = build_resolved_content(&parsed, &["picked line".to_string()]);
        assert_eq!(resolved, "line1\npicked line\nline2\n");
    }

    #[test]
    fn empty_resolution_falls_back_to_ours() {
        let content = "line1\n<<<<<<< HEAD\nours line\n=======\ntheirs line\n>>>>>>> feature\nline2\n";
        let parsed = parse_conflict_content(content);
        let resolved = build_resolved_content(&parsed, &["".to_string()]);
        assert_eq!(resolved, "line1\nours line\nline2\n");
    }

    #[test]
    fn multiple_blocks_stay_in_order() {
        let content = "a\n<<<<<<< H\no1\n=======\nt1\n>>>>>>> f\nmid\n<<<<<<< H\no2\n=======\nt2\n>>>>>>> f\nz\n";
        let parsed = parse_conflict_content(content);
        assert_eq!(parsed.blocks.len(), 2);
        assert_eq!(parsed.merged_parts.len(), 3);
        assert_eq!(parsed.merged_parts[1], "mid\n");
        let resolved = build_resolved_content(&parsed, &["r1".to_string(), "r2".to_string()]);
        assert_eq!(resolved, "a\nr1\nmid\nr2\nz\n");
    }

    #[test]
    fn multiline_block_content_is_preserved() {
        let content = "a\n<<<<<<< H\no1\no2\n=======\nt1\nt2\nt3\n>>>>>>> f\nb\n";
        let parsed = parse_conflict_content(content);
        assert_eq!(parsed.blocks[0].ours_text, "o1\no2");
        assert_eq!(parsed.blocks[0].theirs_text, "t1\nt2\nt3");
        let resolved = build_resolved_content(&parsed, &["x\ny".to_string()]);
        assert_eq!(resolved, "a\nx\ny\nb\n");
    }

    #[test]
    fn diff3_base_section_is_excluded() {
        let content = "a\n<<<<<<< HEAD\nours\n||||||| merged\nbase1\nbase2\n=======\ntheirs\n>>>>>>> f\nb\n";
        let parsed = parse_conflict_content(content);
        assert_eq!(parsed.blocks.len(), 1);
        assert_eq!(parsed.blocks[0].ours_text, "ours");
        assert_eq!(parsed.blocks[0].theirs_text, "theirs");
        let resolved = build_resolved_content(&parsed, &["theirs".to_string()]);
        assert_eq!(resolved, "a\ntheirs\nb\n");
    }

    #[test]
    fn equals_sign_outside_conflict_is_plain_content() {
        let content = "Title\n=======\nbody\n";
        let parsed = parse_conflict_content(content);
        assert!(parsed.blocks.is_empty());
        assert_eq!(parsed.merged_parts.concat(), content);
    }

    #[test]
    fn conflict_at_file_boundaries() {
        let content = "<<<<<<< H\nours\n=======\ntheirs\n>>>>>>> f\n";
        let parsed = parse_conflict_content(content);
        assert_eq!(parsed.blocks.len(), 1);
        assert_eq!(parsed.merged_parts[0], "");
        let resolved = build_resolved_content(&parsed, &["x".to_string()]);
        assert_eq!(resolved, "x\n");
    }

    #[test]
    fn no_conflict_returns_content_unchanged() {
        let content = "just\nsome\nlines\n";
        let parsed = parse_conflict_content(content);
        assert!(parsed.blocks.is_empty());
        assert_eq!(build_resolved_content(&parsed, &[]), content);
    }

    #[test]
    fn resolve_conflict_rewrites_and_stages_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "keep\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n").unwrap();
        let path_str = dir.path().to_str().unwrap().to_string();

        resolve_conflict(path_str, "a.txt".to_string(), vec!["chosen".to_string()]).unwrap();

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep\nchosen\n");
        let index = repo.index().unwrap();
        assert!(index.get_path(Path::new("a.txt"), 0).is_some(), "文件应被加入索引（stage 0）");
    }

    #[test]
    fn resolve_conflict_rejects_stale_block_count() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "keep\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\n").unwrap();
        let path_str = dir.path().to_str().unwrap().to_string();

        let err = resolve_conflict(path_str, "a.txt".to_string(), vec![]).unwrap_err();
        assert!(err.contains("冲突块数量已变化"), "unexpected error: {}", err);

        // 文件不应被修改
        let content = std::fs::read_to_string(&file).unwrap();
        assert!(content.starts_with("keep\n<<<<<<<"));
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    fn commit(repo: &git2::Repository, author: &str, msg: &str, time_secs: i64, files: &[(&str, &str)]) -> String {
        let sig = git2::Signature::new(author, "a@b.c", &git2::Time::new(time_secs, 0)).unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        for (path, content) in files {
            let blob = repo.blob(content.as_bytes()).unwrap();
            tb.insert(path, blob, 0o100644).unwrap();
        }
        let tree_id = tb.write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let owned_parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .map(|c| vec![c])
            .unwrap_or_default();
        let parents: Vec<&git2::Commit> = owned_parents.iter().collect();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap();
        oid.to_string()
    }

    fn setup_repo(dir: &Path) -> (String, String) {
        let repo = git2::Repository::init(dir).unwrap();
        let h1 = commit(&repo, "alice", "Base commit", 1_700_000_000, &[("a.txt", "x\n"), ("b.txt", "y\n")]);
        let h2 = commit(&repo, "alice", "Add lines", 1_700_000_100, &[("a.txt", "x\nx2\n"), ("b.txt", "y\ny2\n")]);
        (h1, h2)
    }

    fn query(dir: &Path, sql: &str) -> Result<QueryResult, String> {
        tauri::async_runtime::block_on(git_query(dir.to_str().unwrap().to_string(), sql.to_string()))
    }

    #[test]
    fn file_changes_table_is_queryable() {
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT COUNT(*) FROM file_changes").unwrap();
        assert_eq!(res.rows, vec![vec!["4".to_string()]], "两笔提交各改两个文件");
    }

    #[test]
    fn per_file_stats_are_not_whole_commit_totals() {
        // 每个文件各自统计，不是整笔提交的总量
        let dir = tempfile::tempdir().unwrap();
        let (_h1, h2) = setup_repo(dir.path());
        let res = query(dir.path(), &format!(
            "SELECT file_path, additions, deletions FROM file_changes WHERE commit_hash = '{}' ORDER BY file_path", h2
        )).unwrap();
        assert_eq!(res.rows, vec![
            vec!["a.txt".to_string(), "1".to_string(), "0".to_string()],
            vec!["b.txt".to_string(), "1".to_string(), "0".to_string()],
        ]);
    }

    #[test]
    fn contains_keyword_and_case_insensitive_eq() {
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT COUNT(*) FROM commits WHERE message CONTAINS 'base'").unwrap();
        assert_eq!(res.rows, vec![vec!["1".to_string()]]);
        let res = query(dir.path(), "SELECT COUNT(*) FROM commits WHERE author = 'ALICE'").unwrap();
        assert_eq!(res.rows, vec![vec!["2".to_string()]]);
    }

    #[test]
    fn aggregates_and_join() {
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT author, COUNT(*) AS cnt FROM commits GROUP BY author").unwrap();
        assert_eq!(res.rows, vec![vec!["alice".to_string(), "2".to_string()]]);
        let res = query(dir.path(),
            "SELECT COUNT(*) FROM commits JOIN file_changes ON commits.hash = file_changes.commit_hash").unwrap();
        assert_eq!(res.rows, vec![vec!["4".to_string()]]);
        let res = query(dir.path(), "SELECT SUM(additions) FROM file_changes").unwrap();
        assert_eq!(res.rows, vec![vec!["4".to_string()]]);
    }

    #[test]
    fn numeric_where_comparison() {
        // 按数值比较，不是字符串
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT COUNT(*) FROM file_changes WHERE additions > 1").unwrap();
        assert_eq!(res.rows, vec![vec!["0".to_string()]]);
        let res = query(dir.path(), "SELECT COUNT(*) FROM file_changes WHERE additions > 0").unwrap();
        assert_eq!(res.rows, vec![vec!["4".to_string()]]);
    }

    #[test]
    fn limit_with_trailing_semicolon() {
        // 分号结尾时 LIMIT 仍生效
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT hash FROM commits LIMIT 1;").unwrap();
        assert_eq!(res.rows.len(), 1);
    }

    #[test]
    fn malformed_sql_returns_error_instead_of_panicking() {
        // 关键词乱序应报错而不是 panic
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        assert!(query(dir.path(), "SELECT hash WHERE message CONTAINS 'x FROM y'").is_err());
        assert!(query(dir.path(), "FROM commits SELECT hash").is_err());
        assert!(query(dir.path(), "SELECT * FROM commits ORDER BY hash WHERE author = 'a'").is_err());
        // 关键词出现在字符串字面量里不应影响解析
        let res = query(dir.path(), "SELECT COUNT(*) FROM commits WHERE message CONTAINS 'select from where'").unwrap();
        assert_eq!(res.rows, vec![vec!["0".to_string()]]);
    }

    #[test]
    fn writes_and_attach_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        assert!(query(dir.path(), "INSERT INTO commits VALUES ('x','y','z','w')").is_err());
        assert!(query(dir.path(), "DELETE FROM commits").is_err());
        assert!(query(dir.path(), "ATTACH DATABASE '/tmp/gitsync-test.db' AS x").is_err());
    }

    #[test]
    fn unknown_table_error_mentions_available_tables() {
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let err = query(dir.path(), "SELECT * FROM nosuch").unwrap_err();
        assert!(err.contains("commits, file_changes"), "err: {}", err);
    }

    #[test]
    fn rewrite_contains_keeps_literals_and_escapes_wildcards() {
        assert_eq!(
            rewrite_contains("WHERE m CONTAINS 'fix'"),
            "WHERE m LIKE '%fix%' ESCAPE '\\'"
        );
        assert_eq!(
            rewrite_contains("SELECT 'a CONTAINS b'"),
            "SELECT 'a CONTAINS b'"
        );
        assert!(rewrite_contains("WHERE m CONTAINS 'a%c_'")
            .contains("LIKE '%a\\%c\\_%' ESCAPE '\\'"));
    }

    #[test]
    fn changelog_with_unparseable_time_does_not_panic() {
        // 时间无法解析时回退「未知时间」；git2 会把时间截断成 32 位，
        // 这里直接写原始 commit 对象绕过创建路径
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let blob = repo.blob(b"z\n").unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        tb.insert("c.txt", blob, 0o100644).unwrap();
        let tree_id = tb.write().unwrap();
        let data = format!(
            "tree {}\nauthor bob <a@b.c> 10000000000000 +0000\ncommitter bob <a@b.c> 10000000000000 +0000\n\nweird time\n",
            tree_id
        );
        let oid = repo.odb().unwrap().write(git2::ObjectType::Commit, data.as_bytes()).unwrap();
        repo.reference("refs/heads/master", oid, true, "test").unwrap();
        repo.set_head("refs/heads/master").unwrap();

        let entries = generate_changelog(dir.path().to_str().unwrap().to_string(), 5).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].version, "未知时间");
    }

    fn query_with(dir: &Path, sql: &str, timeout_ms: u64, row_limit: usize) -> Result<QueryResult, String> {
        git_query_impl(
            dir.to_str().unwrap(),
            sql,
            std::time::Duration::from_millis(timeout_ms),
            row_limit,
        )
    }

    // 40 个提交 × 5 个文件 = 200 条 file_changes，够撑出两表自连接的乘积
    fn repo_with_many_file_changes(dir: &Path) {
        let repo = git2::Repository::init(dir).unwrap();
        for i in 0..40 {
            let files: Vec<(String, String)> = (0..5)
                .map(|f| (format!("f{}.txt", f), format!("v{}\n", i)))
                .collect();
            let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
            commit(&repo, "alice", &format!("c{}", i), 1_700_000_000 + i as i64, &borrowed);
        }
    }

    #[test]
    fn query_row_limit_truncates_instead_of_exhausting_memory() {
        let dir = tempfile::tempdir().unwrap();
        repo_with_many_file_changes(dir.path());

        // 乘积 200^2 = 4 万行，上限 100 应截断并如实标记
        let res = query_with(dir.path(), "SELECT * FROM file_changes a, file_changes b", 10_000, 100).unwrap();
        assert_eq!(res.rows.len(), 100);
        assert!(res.truncated, "命中上限时应标记截断");

        // 未命中上限时不应误报
        let res = query_with(dir.path(), "SELECT COUNT(*) FROM file_changes", 10_000, 100).unwrap();
        assert_eq!(res.rows, vec![vec!["200".to_string()]]);
        assert!(!res.truncated);
    }

    #[test]
    fn query_timeout_interrupts_runaway_join() {
        let dir = tempfile::tempdir().unwrap();
        repo_with_many_file_changes(dir.path());
        // 四路自连接 = 200^4 = 16 亿行，且 count 不产出中间行，行数上限拦不住它
        let err = query_with(
            dir.path(),
            "SELECT COUNT(*) FROM file_changes a, file_changes b, file_changes c, file_changes d",
            300,
            100,
        )
        .unwrap_err();
        assert!(err.contains("已被中止"), "应提示查询被中止: {}", err);
    }

    #[test]
    fn query_rejects_oversized_value() {
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        // 单个值上限挡掉一次性产出几百 MB 的函数
        let err = query_with(dir.path(), "SELECT hex(zeroblob(100000000))", 10_000, 100).unwrap_err();
        assert!(err.contains("上限"), "应提示取值上限: {}", err);
    }
}

#[cfg(test)]
mod path_safety_tests {
    use super::*;

    #[test]
    fn safe_relative_path_accepts_normal_paths() {
        assert_eq!(
            safe_relative_path("pre-commit").unwrap(),
            PathBuf::from("pre-commit")
        );
        assert_eq!(
            safe_relative_path("src/main.rs").unwrap(),
            PathBuf::from("src/main.rs")
        );
    }

    #[test]
    fn safe_relative_path_rejects_traversal() {
        assert!(safe_relative_path("../evil").is_err());
        assert!(safe_relative_path("a/../../evil").is_err());
        // 严格策略：任何 `..` 组件都拒绝（前端只会传普通相对路径）
        assert!(safe_relative_path("sub/../ok").is_err());
        assert!(safe_relative_path("/etc/passwd").is_err());
        assert!(safe_relative_path(".").is_err());
        assert!(safe_relative_path("..").is_err());
        assert!(safe_relative_path("").is_err());
    }

    #[test]
    fn resolve_existing_under_rejects_escape() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("inside.txt"), "x").unwrap();
        // macOS 的 tempdir 带 /var -> /private/var 符号链接，需与 canonicalize 后的 base 比较
        let base = dir.path().canonicalize().unwrap();
        let resolved = resolve_existing_under(dir.path(), Path::new("inside.txt")).unwrap();
        assert!(resolved.starts_with(&base));
        assert!(resolve_existing_under(dir.path(), Path::new("../outside.txt")).is_err());
        assert!(resolve_existing_under(dir.path(), Path::new("missing.txt")).is_err());
    }

    #[test]
    fn hook_commands_reject_traversal_but_keep_normal_flow() {
        let dir = tempfile::tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        std::fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        std::fs::write(dir.path().join(".git/hooks/pre-commit"), "#!/bin/sh\n").unwrap();
        let repo_str = dir.path().to_str().unwrap().to_string();

        // 穿越：读、写都应被拒绝，且目标文件不应出现
        assert!(get_hook_content(repo_str.clone(), "../config".to_string()).is_err());
        assert!(
            save_hook_content(repo_str.clone(), "../../evil.sh".to_string(), "x".to_string()).is_err()
        );
        assert!(!dir.path().parent().unwrap().join("evil.sh").exists());

        // 正常流：读已有 hook、新建 hook 且自动 +x
        assert_eq!(
            get_hook_content(repo_str.clone(), "pre-commit".to_string()).unwrap(),
            "#!/bin/sh\n"
        );
        save_hook_content(repo_str.clone(), "commit-msg".to_string(), "#!/bin/sh\n".to_string())
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(".git/hooks/commit-msg"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "新建 hook 应带执行权限");
        }
    }

    #[test]
    fn get_file_content_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let outside = dir.path().parent().unwrap().join("gitsync-should-not-read.txt");
        std::fs::write(&outside, "secret").unwrap();

        let repo_str = dir.path().to_str().unwrap().to_string();
        assert_eq!(
            get_file_content(repo_str.clone(), "a.txt".to_string()).unwrap(),
            "hello"
        );
        assert!(get_file_content(
            repo_str.clone(),
            format!("../{}", outside.file_name().unwrap().to_string_lossy())
        )
        .is_err());
        assert!(get_file_content(repo_str.clone(), "/etc/passwd".to_string()).is_err());
        let _ = std::fs::remove_file(&outside);
    }

    #[test]
    fn run_script_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let repo_str = dir.path().to_str().unwrap().to_string();
        // 目录外/不存在的脚本一律拒绝，绝不执行
        assert!(run_script_impl(&repo_str, "../../usr/bin/env").is_err());
        assert!(run_script_impl(&repo_str, "/bin/sh").is_err());
    }
}

#[cfg(test)]
mod backend_fix_tests {
    use super::*;

    fn commit(repo: &git2::Repository, msg: &str, time_secs: i64, files: &[(&str, &str)]) -> String {
        commit_by(repo, "alice", msg, time_secs, files)
    }

    fn commit_by(repo: &git2::Repository, author: &str, msg: &str, time_secs: i64, files: &[(&str, &str)]) -> String {
        let sig = git2::Signature::new(author, "a@b.c", &git2::Time::new(time_secs, 0)).unwrap();
        fn build(repo: &git2::Repository, files: &[(&str, &str)]) -> Result<git2::Oid, git2::Error> {
            let mut tb = repo.treebuilder(None)?;
            let mut dirs: std::collections::BTreeMap<String, Vec<(&str, &str)>> = std::collections::BTreeMap::new();
            for (path, content) in files {
                match path.split_once('/') {
                    Some((dir, rest)) => dirs.entry(dir.to_string()).or_default().push((rest, content)),
                    None => {
                        let blob = repo.blob(content.as_bytes())?;
                        tb.insert(path, blob, 0o100644)?;
                    }
                }
            }
            for (dir, sub) in dirs {
                let sub_id = build(repo, &sub)?;
                tb.insert(&dir, sub_id, 0o040000)?;
            }
            tb.write()
        }
        let tree_id = build(repo, files).unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let owned_parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .map(|c| vec![c])
            .unwrap_or_default();
        let parents: Vec<&git2::Commit> = owned_parents.iter().collect();
        let oid = repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents).unwrap();
        oid.to_string()
    }

    #[test]
    fn parse_hunk_header_basic() {
        assert_eq!(parse_hunk_header("@@ -5,7 +5,8 @@ fn main() {"), (5, 7, 5, 8));
        assert_eq!(parse_hunk_header("@@ -0,0 +1,3 @@"), (0, 0, 1, 3));
        assert_eq!(parse_hunk_header("@@ -3 +3 @@"), (3, 1, 3, 1));
    }

    #[test]
    fn diff_detail_splits_two_hunks_with_real_line_counts() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let base_lines: String = (1..=12).map(|i| format!("line{}\n", i)).collect();
        let base = commit(&repo, "base", 1_700_000_000, &[("a.txt", &base_lines)]);
        // 修改第 2 行和第 11 行 → 应产生两个独立 hunk
        let modified = [
            "line1\n", "line2-changed\n", "line3\n", "line4\n", "line5\n", "line6\n",
            "line7\n", "line8\n", "line9\n", "line10\n", "line11-changed\n", "line12\n",
        ]
        .concat();
        let second = commit(&repo, "two changes", 1_700_000_100, &[("a.txt", &modified)]);
        let _ = base;

        let path = dir.path().to_str().unwrap().to_string();
        let files = get_diff_detail(path, second).unwrap();
        assert_eq!(files.len(), 1);
        let (_, detail) = &files[0];
        assert_eq!(detail.hunks.len(), 2, "两处不相邻的修改应切成两个 hunk");
        let h1 = &detail.hunks[0];
        let h2 = &detail.hunks[1];
        assert_eq!(h1.new_start, 1);
        assert_eq!(h1.old_lines, h1.lines.iter().filter(|l| l.origin == "-" || l.origin == " ").count(), "old_lines 应按实际行数统计");
        assert_eq!(h1.new_lines, h1.lines.iter().filter(|l| l.origin == "+" || l.origin == " ").count());
        assert!(h2.old_lines > 0, "old_lines 不应再硬编码为 0");
        assert_eq!(h2.new_start, 8);
    }

    #[test]
    fn deleted_file_reports_old_path_and_content() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let base = commit(&repo, "base", 1_700_000_000, &[("b.txt", "bye\n"), ("keep.txt", "x\n")]);
        let second = commit(&repo, "delete b", 1_700_000_100, &[("keep.txt", "x\n")]);
        let path = dir.path().to_str().unwrap().to_string();

        let detail = tauri::async_runtime::block_on(get_commit_detail(path.clone(), second.clone())).unwrap();
        let deleted = detail.files.iter().find(|f| f.status == "D").expect("应有删除条目");
        assert_eq!(deleted.path, "b.txt", "删除文件应显示原路径而非「未知文件」");

        let files = get_diff_detail(path, second).unwrap();
        let (fp, d) = files.iter().find(|(p, _)| p == "b.txt").expect("应包含 b.txt");
        assert_eq!(fp, "b.txt");
        assert_eq!(d.old_content, "bye\n");
        assert_eq!(d.new_content, "");
        let _ = base;
    }

    #[test]
    fn diff_detail_pairs_consecutive_minus_lines() {
        // 连续删除行都应参与配对，而不是整行高亮
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit(&repo, "base", 1_700_000_000, &[("a.txt", "keep0\nconst x = 1;\nconst y = 2;\nkeep3\n")]);
        let second = commit(&repo, "replace both", 1_700_000_100, &[("a.txt", "keep0\nconst x = 10;\nconst y = 20;\nkeep3\n")]);
        let path = dir.path().to_str().unwrap().to_string();

        let files = get_diff_detail(path, second).unwrap();
        let lines = &files[0].1.hunks[0].lines;
        let minus: Vec<&DiffLine> = lines.iter().filter(|l| l.origin == "-").collect();
        let plus: Vec<&DiffLine> = lines.iter().filter(|l| l.origin == "+").collect();
        assert_eq!(minus.len(), 2, "应有两行删除: {:?}", lines);
        assert_eq!(plus.len(), 2, "应有两行插入: {:?}", lines);
        // -const x = 1 / -const y = 2 与 +const x = 10 / +const y = 20 两两配对，
        // 每行的高亮长度都应小于整行
        for l in minus.iter().chain(plus.iter()) {
            let whole = l.content.trim_end().chars().count();
            let marked: usize = l.inline_changes.iter().map(|c| c.length).sum();
            assert!(marked < whole, "行 {:?} 应有部分级标记而非整行: {:?}", l.content, l.inline_changes);
        }
    }

    #[test]
    fn filter_commits_date_to_is_inclusive() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit(&repo, "day one", 1_760_000_000, &[("a.txt", "1\n")]);
        commit(&repo, "day two", 1_760_086_400, &[("a.txt", "2\n")]);
        let path = dir.path().to_str().unwrap().to_string();

        // 提交时间现按本地时区显示，期望日期从时间戳现场推算，测试在任何时区都成立
        // （两个时间戳相差 24h，本地日期必然不同）
        let day_of = |secs: i64| {
            chrono::Local
                .timestamp_opt(secs, 0)
                .single()
                .unwrap()
                .format("%Y-%m-%d")
                .to_string()
        };
        let res = tauri::async_runtime::block_on(filter_commits(path.clone(), None, None, Some(day_of(1_760_086_400)), None, None)).unwrap();
        assert_eq!(res.len(), 2, "date_to 当天的提交不应被排除");
        let res = tauri::async_runtime::block_on(filter_commits(path.clone(), None, None, Some(day_of(1_760_000_000)), None, None)).unwrap();
        assert_eq!(res.len(), 1);
    }

    #[test]
    fn filter_commits_by_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let base = commit(&repo, "base", 1_700_000_000, &[("a.txt", "1\n"), ("b.txt", "1\n")]);
        let c2 = commit(&repo, "touch a", 1_700_000_100, &[("a.txt", "2\n"), ("b.txt", "1\n")]);
        let c3 = commit(&repo, "touch b", 1_700_000_200, &[("a.txt", "2\n"), ("b.txt", "2\n")]);
        let path = dir.path().to_str().unwrap().to_string();

        // 与 git log -- a.txt 语义一致：创建该文件的提交也算「改过」
        let res = tauri::async_runtime::block_on(filter_commits(path, None, None, None, Some("a.txt".into()), None)).unwrap();
        let hashes: std::collections::HashSet<String> = res.iter().map(|c| c.hash.clone()).collect();
        assert_eq!(hashes, [base, c2].into_iter().collect());
        assert!(!hashes.contains(&c3), "只改 b.txt 的提交不应命中");
    }

    #[test]
    fn semantic_search_reports_real_line_number() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit(&repo, "base", 1_700_000_000, &[("a.txt", "alpha\nbeta\n")]);
        commit(&repo, "add needle at line 3", 1_700_000_100, &[("a.txt", "alpha\nbeta\nneedle\n")]);

        let res = semantic_search_impl(&repo, "needle", false).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].line_number, 3, "应报告真实行号而非匹配序号");
        assert_eq!(res[0].file_path, "a.txt");
        assert_eq!(res[0].time.len(), 19, "时间应为格式化本地时间而非空串: {:?}", res[0].time);
    }

    #[test]
    fn checkout_remote_branch_creates_tracked_local() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        repo.remote("origin", "https://example.com/x.git").unwrap();
        commit(&repo, "base", 1_700_000_000, &[("a.txt", "master\n")]);
        let feat_tip = commit(&repo, "feat", 1_700_000_100, &[("a.txt", "feature\n")]);
        // 建远程跟踪引用（无本地 feat 分支）
        repo.reference("refs/remotes/origin/feat", Oid::from_str(&feat_tip).unwrap(), true, "test").unwrap();

        checkout_branch(dir.path().to_str().unwrap().to_string(), "origin/feat".into()).unwrap();

        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "feat", "HEAD 应指向新建本地分支");
        let local = repo.find_branch("feat", git2::BranchType::Local).unwrap();
        assert_eq!(local.get().peel_to_commit().unwrap().id(), Oid::from_str(&feat_tip).unwrap());
        assert_eq!(local.upstream().unwrap().name().unwrap().unwrap(), "origin/feat");
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "feature\n", "工作区应更新到 feat");
    }

    #[test]
    fn local_time_format_shape() {
        // 只验证格式（YYYY-MM-DD HH:MM:SS，19 字符）与回退路径，不断言具体时区值
        let s = format_local_time(0);
        assert_eq!(s.len(), 19, "格式应为 YYYY-MM-DD HH:MM:SS: {}", s);
        assert!(s.as_bytes()[4] == b'-' && s.as_bytes()[10] == b' ', "格式应为 YYYY-MM-DD HH:MM:SS: {}", s);
        assert_eq!(format_local_time(i64::MAX), "未知时间", "超范围时间应回退");
    }

    #[test]
    fn file_timeline_diff_has_line_prefixes() {
        // 带前缀，否则前端全部丢弃
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit(&repo, "base", 1_700_000_000, &[("a.txt", "alpha\n")]);
        commit(&repo, "add line", 1_700_000_100, &[("a.txt", "alpha\nbeta\n")]);

        let entries = get_file_timeline_impl(&repo, "a.txt").unwrap();
        assert_eq!(entries.len(), 2);
        // entries[0] 是最新提交：+beta。文件头/hunk 头分别带 'F'/'H' 前缀（前端按未分类丢弃，与提交详情一致），
        // 增删行必须带 '+'/'-'/' ' 前缀才会被渲染
        let diff = &entries[0].diff;
        assert!(diff.contains("@@ -1 +1,2 @@"), "应包含 hunk 头内容: {:?}", diff);
        assert!(diff.lines().any(|l| l.starts_with('+')), "新增行应带 + 前缀: {:?}", diff);
        assert!(diff.lines().any(|l| l.starts_with(' ')), "上下文行应带空格前缀: {:?}", diff);
        assert!(entries[1].diff.lines().any(|l| l.starts_with('+')), "首个提交的 diff 也应带前缀");
    }

    #[test]
    fn search_commits_all_refs_finds_side_branch() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let base = commit(&repo, "base", 1_700_000_000, &[("a.txt", "1\n")]);
        let base_commit = repo.find_commit(Oid::from_str(&base).unwrap()).unwrap();
        repo.branch("side", &base_commit, false).unwrap();

        // 在 refs/heads/side 上追加一个只属于该分支的提交（不切换 HEAD）
        let sig = git2::Signature::new("alice", "a@b.c", &git2::Time::new(1_700_000_200, 0)).unwrap();
        let mut tb = repo.treebuilder(Some(&base_commit.tree().unwrap())).unwrap();
        let blob = repo.blob(b"lost\n").unwrap();
        tb.insert("lost.txt", blob, 0o100644).unwrap();
        let tree = repo.find_tree(tb.write().unwrap()).unwrap();
        let lost = repo
            .commit(Some("refs/heads/side"), &sig, &sig, "lost commit", &tree, &[&base_commit])
            .unwrap();

        let path = dir.path().to_str().unwrap().to_string();
        let res = search_commits(path.clone(), "lost commit".into(), None).unwrap();
        assert!(res.is_empty(), "默认 HEAD 视角不应搜到 side 分支的提交");
        let res = search_commits(path.clone(), "lost commit".into(), Some(true)).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].hash, lost.to_string());

        let res = tauri::async_runtime::block_on(filter_commits(
            path, None, None, None, None, Some(true),
        ))
        .unwrap();
        assert!(res.iter().any(|c| c.hash == lost.to_string()), "筛选的全引用模式也应包含该提交");
    }

    #[test]
    fn calendar_counts_commits_by_local_date() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let now = chrono::Utc::now().timestamp();
        // 今天两个、昨天一个、400 天前一个（窗口外）
        commit(&repo, "today a", now, &[("a.txt", "1\n")]);
        commit(&repo, "today b", now, &[("a.txt", "2\n")]);
        commit(&repo, "yesterday", now - 86400, &[("a.txt", "3\n")]);
        commit(&repo, "old", now - 400 * 86400, &[("a.txt", "4\n")]);

        let days = get_commit_calendar_impl(&repo, 365).unwrap();
        let day_of = |secs: i64| {
            chrono::Local
                .timestamp_opt(secs, 0)
                .single()
                .unwrap()
                .format("%Y-%m-%d")
                .to_string()
        };
        let today = day_of(now);
        let got_today = days.iter().find(|d| d.date == today).map(|d| d.count).unwrap_or(0);
        assert_eq!(got_today, 2, "今天应有 2 个提交，实际: {:?}", days);
        assert!(
            !days.iter().any(|d| d.date == day_of(now - 400 * 86400)),
            "窗口外的提交不应出现，实际: {:?}",
            days
        );
    }

    #[test]
    fn knowledge_report_computes_bus_factor() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        // commit() 辅助每次从零建树，后一笔要把前面的文件一起带上，避免整个仓库被「删掉重加」
        let ten: String = (1..=10).map(|i| format!("l{}\n", i)).collect();
        let five: String = (1..=5).map(|i| format!("d{}\n", i)).collect();
        let base_files: &[(&str, &str)] = &[("src/a.txt", &ten)];
        let with_bob: &[(&str, &str)] = &[("src/a.txt", &ten), ("src/b.txt", "x\n")];
        let with_carol: &[(&str, &str)] = &[("src/a.txt", &ten), ("src/b.txt", "x\n"), ("docs/a.md", &five)];
        let with_dave: &[(&str, &str)] = &[
            ("src/a.txt", &ten),
            ("src/b.txt", "x\n"),
            ("docs/a.md", &five),
            ("docs/b.md", &five),
        ];
        commit_by(&repo, "alice", "alice src", 1_700_000_000, base_files);
        commit_by(&repo, "bob", "bob src", 1_700_000_100, with_bob);
        commit_by(&repo, "carol", "carol docs", 1_700_000_200, with_carol);
        commit_by(&repo, "dave", "dave docs", 1_700_000_300, with_dave);

        let report = get_knowledge_report_impl(&repo).unwrap();
        let src = report.iter().find(|d| d.path == "src").expect("应有 src 目录");
        assert_eq!(src.bus_factor, 1, "src 应由 alice 单点维护，实际: {:?}", report);
        assert_eq!(src.top_author, "alice");
        assert!(src.top_share > 0.8, "alice 占比应超过 80%，实际: {}", src.top_share);

        let docs = report.iter().find(|d| d.path == "docs").expect("应有 docs 目录");
        assert_eq!(docs.bus_factor, 2, "docs 需要 carol+dave 两人覆盖过半，实际: {:?}", report);
        assert_eq!(docs.authors, 2);
    }

    #[test]
    fn time_machine_lists_full_paths() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let head = commit(&repo, "nested", 1_700_000_000, &[("docs/guide/intro.md", "hi\n"), ("root.txt", "r\n")]);
        let head_time = 1_700_000_000i64;
        let path = dir.path().to_str().unwrap().to_string();

        let snap = tauri::async_runtime::block_on(get_time_machine_snapshot(path, head_time)).unwrap();
        assert_eq!(snap.commit_hash, head);
        assert!(
            snap.files.iter().any(|f| f.path == "docs/guide/intro.md"),
            "应保留完整相对路径，实际: {:?}",
            snap.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>()
        );
        assert!(snap.files.iter().any(|f| f.path == "docs"));
        // 大小应取自对象头而非 0
        let root = snap.files.iter().find(|f| f.path == "root.txt").expect("应有 root.txt");
        assert_eq!(root.size, 2, "文件大小应取对象头而非 0，实际 {} 字节", root.size);
    }

    fn big_repo(dir: &Path, count: usize) -> Vec<String> {
        let repo = git2::Repository::init(dir).unwrap();
        let mut hashes = Vec::new();
        for i in 0..count {
            let content = format!("v{}\n", i);
            hashes.push(commit(&repo, &format!("msg {}", i), 1_700_000_000 + i as i64, &[("a.txt", &content)]));
        }
        hashes
    }

    #[test]
    fn health_report_scans_head_tree_and_local_stale_branches() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let big: String = std::iter::repeat('x').take(1_500_000).collect();
        commit(&repo, "big", 1_700_000_000, &[("big.bin", &big), ("a.txt", "x\n")]);
        // 建一个本地分支（无上游）和一个远程跟踪引用
        let head_oid = repo.head().unwrap().peel_to_commit().unwrap().id();
        repo.branch("feature", &repo.find_commit(head_oid).unwrap(), false).unwrap();
        repo.reference("refs/remotes/origin/feat", head_oid, true, "test").unwrap();
        // 刚提交的新分支（tip 是刚刚）即使没有上游也不应算废弃；
        // commit() 辅助是从零建树，fresh 提交需带上 big.bin 才能保住大文件断言
        let fresh = commit(&repo, "fresh work", chrono::Utc::now().timestamp(), &[("big.bin", &big), ("a.txt", "x\n"), ("fresh.txt", "1\n")]);
        repo.branch("fresh-branch", &repo.find_commit(Oid::from_str(&fresh).unwrap()).unwrap(), false).unwrap();

        let report = get_health_report_impl(&repo).unwrap();
        assert!(report.large_files.iter().any(|f| f == "big.bin"), "应遍历 HEAD 树发现大文件，实际: {:?}", report.large_files);
        let cur = repo.head().unwrap().shorthand().unwrap().to_string();
        assert!(!report.stale_branches.iter().any(|b| *b == cur), "当前分支不应算废弃分支");
        assert!(report.stale_branches.iter().any(|b| b == "feature"), "长期无提交且无上游的分支应算废弃，实际: {:?}", report.stale_branches);
        assert!(!report.stale_branches.iter().any(|b| b == "fresh-branch"), "新建分支不应算废弃，实际: {:?}", report.stale_branches);
        assert!(!report.stale_branches.iter().any(|b| b.starts_with("origin/")), "远程跟踪分支不应出现在无上游列表");
    }

    #[test]
    fn commits_order_by_topology_under_clock_skew() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        // 父提交时间晚于子提交（时钟偏斜）：拓扑序必须保证子在前
        let parent = commit(&repo, "parent later", 1_700_001_000, &[("a.txt", "1\n")]);
        let child = commit(&repo, "child earlier", 1_700_000_000, &[("a.txt", "2\n")]);
        let path = dir.path().to_str().unwrap().to_string();

        let commits = get_commits(path, None).unwrap();
        assert_eq!(commits[0].hash, child, "子提交应排在前面");
        assert_eq!(commits[1].hash, parent);
        let _ = parent;
    }

    #[test]
    fn pagination_reports_real_total_beyond_100() {
        let dir = tempfile::tempdir().unwrap();
        let hashes = big_repo(dir.path(), 150);
        let path = dir.path().to_str().unwrap().to_string();

        let (page0, total) = tauri::async_runtime::block_on(get_commits_paginated(path.clone(), 0, 30)).unwrap();
        assert_eq!(total, 150, "总数不应被 100 条上限截断");
        assert_eq!(page0.len(), 30);
        assert_eq!(page0[0].hash, hashes[149], "第一页应是最新提交");
        let (page4, _) = tauri::async_runtime::block_on(get_commits_paginated(path.clone(), 4, 30)).unwrap();
        assert_eq!(page4.len(), 30);
        let (page5, _) = tauri::async_runtime::block_on(get_commits_paginated(path.clone(), 5, 30)).unwrap();
        assert_eq!(page5.len(), 0);
    }

    #[test]
    fn limit_param_and_search_beyond_100() {
        let dir = tempfile::tempdir().unwrap();
        let hashes = big_repo(dir.path(), 150);
        let path = dir.path().to_str().unwrap().to_string();

        assert_eq!(get_commits(path.clone(), Some(5)).unwrap().len(), 5);
        assert_eq!(get_commits(path.clone(), None).unwrap().len(), MAIN_COMMIT_LIMIT);

        let res = search_commits(path, "msg 145".into(), None).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].hash, hashes[145], "第 145 条提交应能被搜到（旧实现只见前 100 条）");
    }

    #[test]
    fn contains_handles_escaped_quote() {
        assert_eq!(
            rewrite_contains("WHERE m CONTAINS 'a''b'"),
            "WHERE m LIKE '%a''b%' ESCAPE '\\'"
        );
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit(&repo, "fix: a'b bug", 1_700_000_000, &[("a.txt", "x\n")]);
        let res = tauri::async_runtime::block_on(git_query(
            dir.path().to_str().unwrap().to_string(),
            "SELECT COUNT(*) FROM commits WHERE message CONTAINS 'a''b'".into(),
        )).unwrap();
        assert_eq!(res.rows, vec![vec!["1".to_string()]]);
    }

    #[test]
    fn get_hooks_skips_directories() {
        let dir = tempfile::tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        std::fs::create_dir_all(dir.path().join(".git/hooks/sub.d")).unwrap();
        std::fs::write(dir.path().join(".git/hooks/pre-commit"), "#!/bin/sh\n").unwrap();
        let hooks = get_hooks(dir.path().to_str().unwrap().to_string()).unwrap();
        assert_eq!(hooks, vec!["pre-commit".to_string()]);
    }

    #[test]
    fn inline_changes_are_char_based_and_partial() {
        // 行内 diff 应只标记变化部分，而非整行
        let (del, ins) = paired_inline_changes("const a = 1;", "const a = 2;");
        let whole = "const a = 1;".chars().count();
        assert!(
            del.iter().map(|c| c.length).sum::<usize>() < whole,
            "删除段应小于整行: {:?}", del
        );
        assert!(
            ins.iter().map(|c| c.length).sum::<usize>() < whole,
            "插入段应小于整行: {:?}", ins
        );
        // 偏移量按字符计而非字节：多字节字符后的插入点应为 3（字节会是 9）
        let (_, ins2) = paired_inline_changes("ab 你好", "ab 你好x");
        assert_eq!(ins2[0].offset, 3);
        assert_eq!(ins2[0].length, 3);
    }

    #[test]
    fn diff_detail_provides_paired_inline_changes() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        commit(&repo, "base", 1_700_000_000, &[("a.txt", "const a = 1;\nconst b = 2;\n")]);
        let second = commit(&repo, "change b", 1_700_000_100, &[("a.txt", "const a = 1;\nconst b = 3;\n")]);
        let path = dir.path().to_str().unwrap().to_string();

        let files = get_diff_detail(path, second).unwrap();
        let lines = &files[0].1.hunks[0].lines;
        let minus = lines.iter().find(|l| l.origin == "-").expect("应有删除行");
        let plus = lines.iter().find(|l| l.origin == "+").expect("应有插入行");
        assert_eq!(minus.content.trim_end(), "const b = 2;");
        let whole = minus.content.trim_end().chars().count();
        assert!(
            minus.inline_changes.iter().map(|c| c.length).sum::<usize>() < whole,
            "删除行应有行内级标记而非整行: {:?}", minus.inline_changes
        );
        assert!(
            plus.inline_changes.iter().map(|c| c.length).sum::<usize>() < whole,
            "插入行应有行内级标记而非整行: {:?}", plus.inline_changes
        );
        // 上下文行不带行内标记
        assert!(lines.iter().filter(|l| l.origin == " ").all(|l| l.inline_changes.is_empty()));
    }
}

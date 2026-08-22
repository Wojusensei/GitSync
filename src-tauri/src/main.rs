#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::{Oid, Repository, Sort};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tauri_plugin_dialog::DialogExt;

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

#[tauri::command]
fn get_commits(path: String) -> Result<Vec<Commit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut commits = Vec::new();

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;

        let time = commit.time();
        let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "未知时间".into());

        commits.push(Commit {
            hash: oid.to_string(),
            author: commit.author().name().unwrap_or("未知").to_string(),
            time: timestamp,
            message: commit.message().unwrap_or("").to_string(),
        });

        if commits.len() >= 100 {
            break;
        }
    }

    Ok(commits)
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

    // 先更新工作区，成功后再移动 HEAD，避免失败时分支与工作区不一致
    repo.checkout_tree(&commit.as_object(), Some(&mut git2::build::CheckoutBuilder::default()))
        .map_err(|e| format!("切换分支失败: {}", e))?;

    if track {
        let mut local = repo
            .branch(&target, &commit, false)
            .map_err(|e| format!("创建本地分支失败: {}", e))?;
        local
            .set_upstream(Some(&branch_name))
            .map_err(|e| format!("设置上游失败: {}", e))?;
    }

    repo.set_head(&format!("refs/heads/{}", target))
        .map_err(|e| format!("设置 HEAD 失败: {}", e))?;

    Ok(())
}

#[tauri::command]
fn get_commit_detail(path: String, commit_hash: String) -> Result<CommitDetail, String> {
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

        let path = delta
            .new_file()
            .path()
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
                // 添加行前缀（+, -, 或空格）以便前端正确识别增删行
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

    let time = commit.time();
    let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "未知时间".into());

    Ok(CommitDetail {
        hash: commit_hash,
        author: author_name,
        time: timestamp,
        message,
        files,
    })
}

#[tauri::command]
fn search_commits(path: String, query: String) -> Result<Vec<Commit>, String> {
    let all = get_commits(path)?;
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
fn get_blame(path: String, file_path: String) -> Result<Vec<BlameLine>, String> {
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
        let time = sig.as_ref().map_or("未知时间".into(), |s| {
            let t = s.when();
            chrono::DateTime::from_timestamp(t.seconds(), 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "未知时间".into())
        });

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
fn get_file_timeline(path: String, file_path: String) -> Result<Vec<FileTimelineEntry>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

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
            let delta_path = delta
                .new_file()
                .path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            if delta_path == file_path {
                file_changed = true;
                let patch = git2::Patch::from_diff(&diff, idx)
                    .map_err(|e| format!("Patch 创建失败: {}", e))?;
                if let Some(mut p) = patch {
                    let mut diff_content = Vec::new();
                    p.print(&mut |_delta, _hunk, line| {
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
            let time = commit.time();
            let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "未知时间".into());

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
fn get_health_report(path: String) -> Result<HealthReport, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut large_files = Vec::new();
    let mut conflicts = Vec::new();

    if let Ok(statuses) = repo.statuses(None) {
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("未知");
            if entry.status() == git2::Status::CONFLICTED {
                conflicts.push(path.to_string());
            }
            if let Ok(tree) = repo.head().and_then(|h| h.peel_to_tree()) {
                if let Ok(entry) = tree.get_path(Path::new(path)) {
                    let blob = repo.find_blob(entry.id()).ok();
                    if let Some(blob) = blob {
                        if blob.size() > 1024 * 1024 {
                            large_files.push(path.to_string());
                        }
                    }
                }
            }
        }
    }

    let mut stale_branches = Vec::new();
    if let Ok(branches) = repo.branches(None) {
        for branch in branches {
            if let Ok((branch, _)) = branch {
                let name = branch.name().map_err(|e| format!("分支名错误: {}", e))?;
                let name = name.unwrap_or("未知").to_string();
                if branch.upstream().is_err() {
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

#[derive(Serialize)]
struct Contributor {
    author: String,
    email: String,
    commits: usize,
    additions: usize,
    deletions: usize,
}

#[tauri::command]
fn get_contributors(path: String) -> Result<Vec<Contributor>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

    let mut contributors: std::collections::HashMap<String, (String, usize, usize, usize)> = std::collections::HashMap::new();

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

        if contributors.len() >= 50 {
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
fn get_hot_files(path: String) -> Result<Vec<HotFile>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

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
    let commits = get_commits(path)?;
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
fn execute_rebase(path: String, operations: Vec<RebaseOperation>) -> Result<String, String> {
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
fn semantic_search(path: String, query: String) -> Result<Vec<SearchResult>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

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
            let path = delta.new_file().path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

            let patch = git2::Patch::from_diff(&diff, idx)
                .map_err(|e| format!("Patch 创建失败: {}", e))?;
            if let Some(mut p) = patch {
                let mut line_number = 0usize;
                p.print(&mut |_delta, _hunk, line| {
                    let content = std::str::from_utf8(line.content()).unwrap_or("");
                    if content.to_lowercase().contains(&query_lower) {
                        line_number += 1;
                        results.push(SearchResult {
                            commit_hash: oid.to_string(),
                            author: commit.author().name().unwrap_or("未知").to_string(),
                            time: "".to_string(),
                            file_path: path.clone(),
                            line_number,
                            content: content.to_string(),
                        });
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
fn compare_commits(path: String, commit_a: String, commit_b: String) -> Result<DiffResult, String> {
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
    let commits = get_commits(path)?;
    let mut entries: Vec<ChangelogEntry> = Vec::new();

    let mut current_date = String::new();
    let mut current_messages = Vec::new();

    for commit in commits.iter().take(count) {
        let date = &commit.time[..10];
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
fn get_graph_commits(path: String) -> Result<Vec<GraphCommit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

    let mut results = Vec::new();
    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let time = commit.time();
        let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "未知时间".into());

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
fn get_file_tree(path: String) -> Result<Vec<TreeNode>, String> {
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
fn filter_commits(path: String, author: Option<String>, date_from: Option<String>, date_to: Option<String>, _file_path: Option<String>) -> Result<Vec<Commit>, String> {
    let all = get_commits(path)?;
    let filtered: Vec<Commit> = all.into_iter().filter(|c| {
        if let Some(ref a) = author { if !c.author.to_lowercase().contains(&a.to_lowercase()) { return false; } }
        if let Some(ref df) = date_from { if c.time < *df { return false; } }
        if let Some(ref dt) = date_to { if c.time > *dt { return false; } }
        true
    }).collect();
    Ok(filtered)
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

#[derive(Serialize, Clone)]
struct InlineChange {
    offset: usize,
    length: usize,
    kind: String,
}
#[derive(Serialize, Clone)]
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
// 修正后的 inline changes 计算函数
fn compute_inline_changes(old_line: &str, new_line: &str) -> Vec<InlineChange> {
    use similar::TextDiff;
    let mut changes = Vec::new();
    // 按字符分词比较
    let diff = TextDiff::from_words(old_line, new_line);
    let mut offset = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Equal => {
                let text = change.as_str().unwrap_or("");
                offset += text.len();
            }
            similar::ChangeTag::Delete => {
                let text = change.as_str().unwrap_or("");
                changes.push(InlineChange {
                    offset,
                    length: text.len(),
                    kind: "delete".to_string(),
                });
                // 删除不增加 offset，因为它是删除的字符，在结果中不占位
            }
            similar::ChangeTag::Insert => {
                let text = change.as_str().unwrap_or("");
                changes.push(InlineChange {
                    offset,
                    length: text.len(),
                    kind: "insert".to_string(),
                });
                offset += text.len();
            }
        }
    }
    changes
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
        let file_path = delta.new_file().path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let old_content = if let Some(parent_tree) = parent_tree.as_ref() {
            parent_tree.get_path(Path::new(&file_path)).ok().and_then(|e| repo.find_blob(e.id()).ok()).map(|b| String::from_utf8_lossy(b.content()).to_string()).unwrap_or_default()
        } else { String::new() };
        let new_content = tree.get_path(Path::new(&file_path)).ok().and_then(|e| repo.find_blob(e.id()).ok()).map(|b| String::from_utf8_lossy(b.content()).to_string()).unwrap_or_default();

        let patch = git2::Patch::from_diff(&diff, idx).map_err(|e| format!("Patch 创建失败: {}", e))?;
        let mut hunks = Vec::new();
        if let Some(mut p) = patch {
            let mut current_old_start = 0;
            let mut current_new_start = 0;
            let mut current_lines = Vec::new();

            p.print(&mut |_delta, hunk, line| {
                if let Some(hunk) = hunk {
                    if !current_lines.is_empty() {
                        hunks.push(DiffHunk {
                            old_start: current_old_start,
                            old_lines: 0,
                            new_start: current_new_start,
                            new_lines: 0,
                            lines: current_lines.clone(),
                        });
                        current_lines.clear();
                    }
                    let header = String::from_utf8_lossy(hunk.header()).to_string();
                    let (os, _ol, ns, _nl) = parse_hunk_header(&header);
                    current_old_start = os;
                    current_new_start = ns;
                } else {
                    let origin = match line.origin() {
                        '+' => "+",
                        '-' => "-",
                        ' ' => " ",
                        _ => "?",
                    };
                    let content = String::from_utf8_lossy(line.content()).to_string();
                    let inline_changes = if origin == "+" || origin == "-" {
                        let old_line = if origin == "+" { "" } else { &content };
                        let new_line = if origin == "+" { &content } else { "" };
                        compute_inline_changes(old_line, new_line)
                    } else {
                        vec![]
                    };
                    current_lines.push(DiffLine {
                        origin: origin.to_string(),
                        content: content.clone(),
                        inline_changes,
                    });
                }
                true
            }).map_err(|e| format!("Diff 打印失败: {}", e))?;

            if !current_lines.is_empty() {
                hunks.push(DiffHunk {
                    old_start: current_old_start,
                    old_lines: 0,
                    new_start: current_new_start,
                    new_lines: 0,
                    lines: current_lines,
                });
            }
        }
        results.push((file_path, DiffDetail { old_content, new_content, hunks }));
    }
    Ok(results)
}

fn parse_hunk_header(header: &str) -> (usize, usize, usize, usize) {
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() < 4 { return (0, 0, 0, 0); }
    let old = parts[0].trim_start_matches("@@").trim();
    let new = parts[2].trim();
    let old_parts: Vec<&str> = old.split(',').collect();
    let new_parts: Vec<&str> = new.split(',').collect();
    let old_start = old_parts[0].parse::<isize>().unwrap_or(0).unsigned_abs();
    let old_lines = if old_parts.len() > 1 { old_parts[1].parse().unwrap_or(0) } else { 1 };
    let new_start = new_parts[0].parse::<isize>().unwrap_or(0).unsigned_abs();
    let new_lines = if new_parts.len() > 1 { new_parts[1].parse().unwrap_or(0) } else { 1 };
    (old_start, old_lines, new_start, new_lines)
}

#[tauri::command]
fn get_commits_paginated(path: String, page: usize, page_size: usize) -> Result<(Vec<Commit>, usize), String> {
    let all = get_commits(path)?;
    let total = all.len();
    let start = page * page_size;
    if start >= total {
        return Ok((vec![], total));
    }
    let end = (start + page_size).min(total);
    let page_data = all[start..end].to_vec();
    Ok((page_data, total))
}

#[tauri::command]
fn get_hooks(path: String) -> Result<Vec<String>, String> {
    let hooks_dir = format!("{}/.git/hooks", shellexpand::tilde(&path));
    let mut hooks = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&hooks_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".sample") { hooks.push(name); }
        }
    }
    Ok(hooks)
}

#[tauri::command]
fn get_hook_content(path: String, hook_name: String) -> Result<String, String> {
    let hook_path = format!("{}/.git/hooks/{}", shellexpand::tilde(&path), hook_name);
    std::fs::read_to_string(&hook_path).map_err(|e| format!("读取失败: {}", e))
}

#[tauri::command]
fn save_hook_content(path: String, hook_name: String, content: String) -> Result<(), String> {
    let hook_path = format!("{}/.git/hooks/{}", shellexpand::tilde(&path), hook_name);
    std::fs::write(&hook_path, content).map_err(|e| format!("写入失败: {}", e))?;
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
    // 每个冲突块之前的普通文本；长度恒为 blocks.len() + 1，最后一段是文件末尾的普通文本
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

// 把每块的解决文本插回冲突原来的位置；空解决文本回退到 ours（与原有交互语义一致）
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
    let full_path = Path::new(&expanded).join(&file_path);
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
fn export_report_markdown(path: String) -> Result<String, String> {
    let health = get_health_report(path.clone())?;
    let contributors = get_contributors(path.clone())?;
    let hot_files = get_hot_files(path.clone())?;

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

#[tauri::command]
fn run_script(path: String, script_name: String) -> Result<String, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let scripts_dir = shellexpand::tilde("~/.git-tool/scripts").to_string();
    let script_path = format!("{}/{}", scripts_dir, script_name);
    
    let output = std::process::Command::new(&script_path)
        .arg(&expanded)
        .output()
        .map_err(|e| format!("执行脚本失败: {}", e))?;
    
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ====================
// SQL 查询引擎 v2 — 微型数据库
// ====================

#[derive(Serialize)]
struct QueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    elapsed_ms: u64,
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
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

    let mut commits = Vec::new();
    let mut files = Vec::new();

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let tree = commit.tree().map_err(|e| format!("无法获取树: {}", e))?;
        let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());

        let time = commit.time();
        let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "未知时间".into());

        let hash = oid.to_string();
        commits.push(Commit {
            hash: hash.clone(),
            author: commit.author().name().unwrap_or("未知").to_string(),
            time: timestamp,
            message: commit.message().unwrap_or("").to_string(),
        });

        if let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            for delta in diff.deltas() {
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
                let (additions, deletions) = if let Ok(Some(_patch)) = git2::Patch::from_diff(&diff, 0) {
                    let mut add = 0;
                    let mut del = 0;
                    // 简化：对整个 patch 遍历统计
                    if let Ok(_) = diff.foreach(
                        &mut |_, _| true,
                        None,
                        None,
                        Some(&mut |_, _, line| {
                            match line.origin() {
                                '+' => add += 1,
                                '-' => del += 1,
                                _ => {}
                            }
                            true
                        }),
                    ) {
                        (add, del)
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                };

                files.push(FileChangeRow {
                    commit_hash: hash.clone(),
                    file_path,
                    status: status.to_string(),
                    additions,
                    deletions,
                });
            }
        }

        if commits.len() >= 200 { break; }
    }

    Ok((commits, files))
}

// 解析聚合函数，返回 (函数名, 列名)
fn parse_aggregate(expr: &str) -> Option<(String, String)> {
    let expr = expr.trim();
    let upper = expr.to_uppercase();
    for func in &["COUNT", "SUM", "AVG", "MAX", "MIN"] {
        if upper.starts_with(func) {
            let inner = expr[func.len()..].trim().trim_start_matches('(').trim_end_matches(')').trim();
            if inner == "*" {
                return Some((func.to_string(), "*".into()));
            }
            return Some((func.to_string(), inner.to_string()));
        }
    }
    None
}

// 匹配 WHERE 条件
fn match_filter(val: &str, op: &str, target: &str) -> bool {
    match op {
        "=" => val.to_lowercase() == target.to_lowercase(),
        ">" => *val > *target,
        "<" => *val < *target,
        "CONTAINS" => val.to_lowercase().contains(&target.to_lowercase()),
        _ => false,
    }
}

#[tauri::command]
fn git_query(path: String, sql: String) -> Result<QueryResult, String> {
    let start = std::time::Instant::now();

    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let (commits, file_changes) = collect_all_data(&repo)?;

    // 解析 SQL
    let sql_upper = sql.to_uppercase();
    let select_idx = sql_upper.find("SELECT").ok_or("SQL 语法错误: 缺少 SELECT")?;
    let from_idx = sql_upper.find("FROM").ok_or("SQL 语法错误: 缺少 FROM")?;
    let join_idx = sql_upper.find("JOIN");
    let where_idx = sql_upper.find("WHERE");
    let group_idx = sql_upper.find("GROUP BY");
    let order_idx = sql_upper.find("ORDER BY");
    let limit_idx = sql_upper.find("LIMIT");

    // 表名
    let from_end = join_idx.unwrap_or(where_idx.unwrap_or(group_idx.unwrap_or(order_idx.unwrap_or(limit_idx.unwrap_or(sql.len())))));
    let from_part = &sql[from_idx + 4..from_end].trim().to_lowercase();
    let main_table = from_part.as_str();
    let mut join_table: Option<&str> = None;
    let mut join_condition: Option<(String, String)> = None; // (left, right)

    if let Some(ji) = join_idx {
        let join_end = where_idx.unwrap_or(group_idx.unwrap_or(order_idx.unwrap_or(limit_idx.unwrap_or(sql.len()))));
        let join_part = &sql[ji..join_end].trim();
        let parts: Vec<&str> = join_part.split_whitespace().collect();
        if parts.len() >= 4 {
            join_table = Some(parts[1]);
            let on_idx = join_part.to_uppercase().find("ON");
            if let Some(oi) = on_idx {
                let cond = &join_part[oi + 2..].trim();
                let cond_parts: Vec<&str> = cond.split('=').collect();
                if cond_parts.len() == 2 {
                    join_condition = Some((cond_parts[0].trim().to_string(), cond_parts[1].trim().to_string()));
                }
            }
        }
    }

    // 验证表名
    let valid_tables = ["commits", "file_changes"];
    if !valid_tables.contains(&main_table) {
        return Err(format!("表 \"{}\" 不存在，可用表: commits, file_changes", main_table));
    }
    if let Some(jt) = join_table {
        if !valid_tables.contains(&jt) {
            return Err(format!("表 \"{}\" 不存在，可用表: commits, file_changes", jt));
        }
    }

    // SELECT 列
    let select_part = &sql[select_idx + 6..from_idx].trim();
    let mut columns: Vec<String> = Vec::new();
    let mut aggregates: Vec<Option<(String, String)>> = Vec::new(); // 每个列对应的聚合函数

    if *select_part == "*" {
        if main_table == "commits" {
            columns = vec!["hash".into(), "author".into(), "time".into(), "message".into()];
        } else {
            columns = vec!["commit_hash".into(), "file_path".into(), "status".into(), "additions".into(), "deletions".into()];
        }
    } else {
        for col_expr in select_part.split(',') {
            let col_expr = col_expr.trim();
            // 检查是否有 AS 别名
            let (col_name, alias) = if let Some(as_idx) = col_expr.to_uppercase().find(" AS ") {
                let name_part = col_expr[..as_idx].trim().to_lowercase();
                let alias_part = col_expr[as_idx + 4..].trim();
                (name_part, alias_part.to_string())
            } else {
                (col_expr.to_lowercase(), col_expr.to_string())
            };

            let agg = parse_aggregate(&col_name);
            aggregates.push(agg.clone());
            if let Some((_func, _inner)) = &agg {
                columns.push(alias);
            } else {
                columns.push(alias);
            }
        }
    }

    // WHERE 条件
    let mut filters: Vec<(String, String, String)> = Vec::new(); // (column, op, value)
    if let Some(wi) = where_idx {
        let where_end = group_idx.unwrap_or(order_idx.unwrap_or(limit_idx.unwrap_or(sql.len())));
        let where_part = &sql[wi + 5..where_end].trim();
        if !where_part.is_empty() {
            let conditions: Vec<&str> = where_part.split("AND").map(|s| s.trim()).collect();
            for cond in conditions {
                let cond = cond.trim();
                if let Some(val) = cond.split(" = ").nth(1) {
                    let col = cond.split(" = ").next().unwrap().trim().to_lowercase();
                    filters.push((col, "=".into(), val.trim().trim_matches('\'').to_string()));
                } else if let Some(val) = cond.split(" > ").nth(1) {
                    let col = cond.split(" > ").next().unwrap().trim().to_lowercase();
                    filters.push((col, ">".into(), val.trim().trim_matches('\'').to_string()));
                } else if let Some(val) = cond.split(" < ").nth(1) {
                    let col = cond.split(" < ").next().unwrap().trim().to_lowercase();
                    filters.push((col, "<".into(), val.trim().trim_matches('\'').to_string()));
                } else if cond.to_uppercase().contains("CONTAINS") {
                    let parts: Vec<&str> = cond.split("CONTAINS").collect();
                    if parts.len() == 2 {
                        let col = parts[0].trim().to_lowercase();
                        let val = parts[1].trim().trim_matches('\'').to_string();
                        filters.push((col, "CONTAINS".into(), val));
                    }
                }
            }
        }
    }

    // 过滤主表数据
    let filtered_commits: Vec<&Commit> = if main_table == "commits" {
        commits.iter().filter(|c| {
            filters.iter().all(|(col, op, val)| {
                match col.as_str() {
                    "author" => match_filter(&c.author, op, val),
                    "hash" => match_filter(&c.hash, op, val),
                    "time" => match_filter(&c.time, op, val),
                    "message" => match_filter(&c.message, op, val),
                    _ => true,
                }
            })
        }).collect()
    } else {
        vec![]
    };

    // JOIN 逻辑
    let mut joined_rows: Vec<HashMap<String, String>> = Vec::new();
    if let (Some(_jt), Some((left, right))) = (join_table, &join_condition) {
        let left = left.replace("commits.", "").replace("file_changes.", "");
        let right = right.replace("commits.", "").replace("file_changes.", "");
        for c in &filtered_commits {
            for f in &file_changes {
                let left_val = if left == "hash" || left == "commit_hash" { &c.hash } else { "" };
                let right_val = if right == "hash" || right == "commit_hash" { &f.commit_hash } else if right == "file_path" { &f.file_path } else { "" };
                if left_val == right_val {
                    let mut row = HashMap::new();
                    row.insert("hash".into(), c.hash.clone());
                    row.insert("author".into(), c.author.clone());
                    row.insert("time".into(), c.time.clone());
                    row.insert("message".into(), c.message.clone());
                    row.insert("commit_hash".into(), f.commit_hash.clone());
                    row.insert("file_path".into(), f.file_path.clone());
                    row.insert("status".into(), f.status.clone());
                    row.insert("additions".into(), f.additions.to_string());
                    row.insert("deletions".into(), f.deletions.to_string());
                    joined_rows.push(row);
                }
            }
        }
    } else {
        for c in &filtered_commits {
            let mut row = HashMap::new();
            row.insert("hash".into(), c.hash.clone());
            row.insert("author".into(), c.author.clone());
            row.insert("time".into(), c.time.clone());
            row.insert("message".into(), c.message.clone());
            joined_rows.push(row);
        }
    }

    // GROUP BY 和聚合
    let group_col = if let Some(gi) = group_idx {
        let group_end = order_idx.unwrap_or(limit_idx.unwrap_or(sql.len()));
        Some(sql[gi + 8..group_end].trim().to_lowercase())
    } else {
        None
    };

    let mut result_rows: Vec<Vec<String>> = Vec::new();

    if let Some(gcol) = group_col {
        let mut groups: HashMap<String, Vec<&HashMap<String, String>>> = HashMap::new();
        for row in &joined_rows {
            if let Some(val) = row.get(&gcol) {
                groups.entry(val.clone()).or_insert_with(Vec::new).push(row);
            }
        }
        for (_key, group_rows) in &groups {
            let mut result_row: Vec<String> = Vec::new();
            for (i, col) in columns.iter().enumerate() {
                if let Some(Some((func, inner))) = aggregates.get(i) {
                    let vals: Vec<f64> = group_rows.iter().filter_map(|r| {
                        if inner == "*" {
                            Some(1.0)
                        } else {
                            r.get(inner).and_then(|v| v.parse::<f64>().ok())
                        }
                    }).collect();
                    let val = match func.as_str() {
                        "COUNT" => vals.len() as f64,
                        "SUM" => vals.iter().sum(),
                        "AVG" => if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 },
                        "MAX" => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                        "MIN" => vals.iter().cloned().fold(f64::INFINITY, f64::min),
                        _ => 0.0,
                    };
                    result_row.push(val.to_string());
                } else if let Some(val) = group_rows.first().and_then(|r| r.get(col)) {
                    result_row.push(val.clone());
                } else {
                    result_row.push(String::new());
                }
            }
            result_rows.push(result_row);
        }
    } else {
        for row in &joined_rows {
            let mut result_row: Vec<String> = Vec::new();
            for col in &columns {
                if let Some(val) = row.get(col) {
                    result_row.push(val.clone());
                } else {
                    result_row.push(String::new());
                }
            }
            result_rows.push(result_row);
        }
    }

    // ORDER BY
    if let Some(oi) = order_idx {
        let order_end = limit_idx.unwrap_or(sql.len());
        let order_part = &sql[oi + 8..order_end].trim();
        let desc = order_part.to_uppercase().contains("DESC");
        let order_col = order_part.replace(" DESC", "").replace(" ASC", "").trim().to_lowercase();
        if let Some(col_idx) = columns.iter().position(|c| c == &order_col) {
            result_rows.sort_by(|a, b| {
                let va = a.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let vb = b.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                if desc { vb.cmp(&va) } else { va.cmp(&vb) }
            });
        }
    }

    // LIMIT
    if let Some(li) = limit_idx {
        let limit_part = &sql[li + 5..].trim();
        if let Ok(limit) = limit_part.parse::<usize>() {
            result_rows.truncate(limit);
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;
    Ok(QueryResult {
        columns,
        rows: result_rows,
        elapsed_ms: elapsed,
    })
}

// ====================
// Git 时间机器
// ====================

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
    let full_path = Path::new(&expanded).join(&file_path);
    if !full_path.exists() {
        return Err("文件不存在".to_string());
    }
    let content = std::fs::read(full_path).map_err(|e| format!("无法读取文件: {}", e))?;
    Ok(String::from_utf8_lossy(&content).to_string())
}

#[tauri::command]
fn get_time_machine_snapshot(path: String, timestamp: i64) -> Result<TimeMachineSnapshot, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

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
    let commit_time = commit.time();
    let timestamp_str = chrono::DateTime::from_timestamp(commit_time.seconds(), 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "未知时间".into());
    let message = commit.message().unwrap_or("").to_string();

    let mut files = Vec::new();
    tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
        let name = entry.name().unwrap_or("未知");
        let kind = entry.kind();
        let is_dir = kind == Some(git2::ObjectType::Tree);
        let size = if is_dir {
            0
        } else {
            repo.find_blob(entry.id()).map(|b| b.size()).unwrap_or(0)
        };
        files.push(TimeMachineFile {
            path: name.to_string(),
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
    let file_path = app.dialog().file()
        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
        .blocking_pick_file();
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
    let folder_path = app.dialog().file().blocking_pick_folder();
    match folder_path {
        Some(path) => {
            let p = path.as_path().ok_or("路径无效")?;
            Ok(p.to_string_lossy().to_string())
        }
        None => Err("用户取消了选择".into()),
    }
}

fn main() {
    unsafe {
        let _ = git2::opts::set_verify_owner_validation(false);
    }
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
            git_query
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

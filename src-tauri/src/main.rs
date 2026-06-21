#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::{Oid, Repository, Sort};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
        let is_head = head_name.as_deref() == Some(&name);
        result.push(Branch { name, is_head });
    }

    result.sort_by(|a, b| b.is_head.cmp(&a.is_head));

    Ok(result)
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

    let mut contributors: std::collections::HashMap<String, (usize, usize, usize)> = std::collections::HashMap::new();

    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let author = commit.author().name().unwrap_or("未知").to_string();

        let entry = contributors.entry(author).or_insert((0, 0, 0));
        entry.0 += 1;

        if let Ok(tree) = commit.tree() {
            let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
            if let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
                diff.foreach(
                    &mut |_delta, _| true,
                    None,
                    None,
                    Some(&mut |_delta, _hunk, line| {
                        match line.origin() {
                            '+' => entry.1 += 1,
                            '-' => entry.2 += 1,
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

    let result: Vec<Contributor> = contributors
        .into_iter()
        .map(|(author, (commits, additions, deletions))| Contributor {
            author,
            commits,
            additions,
            deletions,
        })
        .collect();

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

    let mut todo_content = String::new();
    for op in &operations {
        let line = match op.action.as_str() {
            "squash" => format!("squash {}", &op.hash[..8]),
            "drop" => format!("drop {}", &op.hash[..8]),
            "reword" => format!("reword {}", &op.hash[..8]),
            _ => format!("pick {}", &op.hash[..8]),
        };
        todo_content.push_str(&line);
        todo_content.push('\n');
    }

    let temp_file = format!("{}/.git-rebase-todo", expanded);
    std::fs::write(&temp_file, todo_content).map_err(|e| format!("写入 todo 文件失败: {}", e))?;

    let output = std::process::Command::new("git")
        .current_dir(&expanded)
        .env("GIT_SEQUENCE_EDITOR", format!("cat {}", temp_file))
        .arg("rebase")
        .arg("-i")
        .arg(format!("HEAD~{}", operations.len()))
        .output()
        .map_err(|e| format!("执行 rebase 失败: {}", e))?;

    std::fs::remove_file(temp_file).ok();

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok("rebase 成功".into())
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

#[derive(Serialize)]
struct DiffDetail {
    old_content: String,
    new_content: String,
    hunks: Vec<DiffHunk>,
}

#[derive(Serialize)]
struct DiffHunk {
    old_start: usize,
    old_lines: usize,
    new_start: usize,
    new_lines: usize,
    lines: Vec<DiffLine>,
}

#[derive(Serialize, Clone)]
struct DiffLine {
    origin: String,
    content: String,
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
                    current_lines.push(DiffLine {
                        origin: match line.origin() {
                            '+' => "+".into(),
                            '-' => "-".into(),
                            ' ' => " ".into(),
                            _ => "?".into(),
                        },
                        content: String::from_utf8_lossy(line.content()).to_string(),
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

#[tauri::command]
fn get_conflict_detail(path: String) -> Result<Vec<ConflictDetail>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut conflict_files = Vec::new();
    
    if let Ok(statuses) = repo.statuses(None) {
        for entry in statuses.iter() {
            if entry.status() == git2::Status::CONFLICTED {
                let file_path = entry.path().unwrap_or("未知").to_string();
                let full_path = format!("{}/{}", expanded, file_path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let mut ours = String::new();
                    let mut theirs = String::new();
                    let mut merged = String::new();
                    let mut blocks = Vec::new();
                    let mut in_ours = false;
                    let mut in_theirs = false;
                    let mut ours_lines = Vec::new();
                    let mut theirs_lines = Vec::new();
                    
                    for line in content.lines() {
                        if line.starts_with("<<<<<<<") {
                            in_ours = true;
                            continue;
                        } else if line.starts_with("=======") {
                            in_ours = false;
                            in_theirs = true;
                            continue;
                        } else if line.starts_with(">>>>>>>") {
                            in_theirs = false;
                            blocks.push(ConflictBlock {
                                ours_text: ours_lines.join("\n"),
                                theirs_text: theirs_lines.join("\n"),
                            });
                            ours_lines.clear();
                            theirs_lines.clear();
                            continue;
                        }
                        
                        if in_ours {
                            ours_lines.push(line.to_string());
                        } else if in_theirs {
                            theirs_lines.push(line.to_string());
                        } else {
                            merged.push_str(line);
                            merged.push('\n');
                        }
                    }
                    
                    ours = blocks.iter().map(|b| b.ours_text.as_str()).collect::<Vec<&str>>().join("\n");
                    theirs = blocks.iter().map(|b| b.theirs_text.as_str()).collect::<Vec<&str>>().join("\n");
                    
                    conflict_files.push(ConflictDetail {
                        path: file_path,
                        ours,
                        theirs,
                        merged,
                        conflict_blocks: blocks,
                    });
                }
            }
        }
    }
    Ok(conflict_files)
}

#[tauri::command]
fn resolve_conflict(path: String, file_path: String, resolution: String) -> Result<(), String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let full_path = format!("{}/{}", expanded, file_path);
    std::fs::write(&full_path, resolution).map_err(|e| format!("写入失败: {}", e))?;
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
// SQL 查询引擎
// ====================

#[derive(Serialize)]
struct QueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    elapsed_ms: u64,
}

#[tauri::command]
fn git_query(path: String, sql: String) -> Result<QueryResult, String> {
    let start = std::time::Instant::now();

    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let sql_upper = sql.to_uppercase();
    let select_idx = sql_upper.find("SELECT").ok_or("需要 SELECT 语句")?;
    let from_idx = sql_upper.find("FROM").ok_or("需要 FROM 子句")?;
    let where_idx = sql_upper.find("WHERE");
    let order_idx = sql_upper.find("ORDER BY");
    let limit_idx = sql_upper.find("LIMIT");

    let select_part = &sql[select_idx + 6..from_idx].trim();
    let columns: Vec<String> = if *select_part == "*" {
        vec!["hash".into(), "author".into(), "time".into(), "message".into()]
    } else {
        select_part.split(',').map(|s| s.trim().to_lowercase()).collect()
    };

    let from_part = &sql[from_idx + 4..where_idx.unwrap_or(order_idx.unwrap_or(limit_idx.unwrap_or(sql.len())))].trim();
    if from_part.to_lowercase() != "commits" {
        return Err("目前只支持 FROM commits".into());
    }

    let mut author_filter: Option<String> = None;
    let mut time_after: Option<String> = None;
    let mut time_before: Option<String> = None;
    let mut message_contains: Option<String> = None;

    if let Some(wi) = where_idx {
        let where_end = order_idx.unwrap_or(limit_idx.unwrap_or(sql.len()));
        let where_part = &sql[wi + 5..where_end].trim();
        let conditions: Vec<&str> = where_part.split("AND").map(|s| s.trim()).collect();

        for cond in conditions {
            let cond_upper = cond.to_uppercase();
            if cond_upper.contains("AUTHOR =") {
                if let Some(val) = cond.split('=').nth(1) {
                    author_filter = Some(val.trim().trim_matches('\'').to_string());
                }
            } else if cond_upper.contains("TIME >") {
                if let Some(val) = cond.split('>').nth(1) {
                    time_after = Some(val.trim().trim_matches('\'').to_string());
                }
            } else if cond_upper.contains("TIME <") {
                if let Some(val) = cond.split('<').nth(1) {
                    time_before = Some(val.trim().trim_matches('\'').to_string());
                }
            } else if cond_upper.contains("MESSAGE CONTAINS") || cond_upper.contains("MESSAGE LIKE") {
                if let Some(val) = cond.split("CONTAINS").nth(1).or_else(|| cond.split("LIKE").nth(1)) {
                    message_contains = Some(val.trim().trim_matches('\'').trim_matches('%').to_string());
                }
            }
        }
    }

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

    let mut all_commits: Vec<Commit> = Vec::new();
    for oid in revwalk {
        let oid = oid.map_err(|e| format!("遍历失败: {}", e))?;
        let commit = repo.find_commit(oid).map_err(|e| format!("找不到提交: {}", e))?;
        let time = commit.time();
        let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "未知时间".into());

        all_commits.push(Commit {
            hash: oid.to_string(),
            author: commit.author().name().unwrap_or("未知").to_string(),
            time: timestamp,
            message: commit.message().unwrap_or("").to_string(),
        });

        if all_commits.len() >= 200 { break; }
    }

    let filtered: Vec<Commit> = all_commits.into_iter().filter(|c| {
        if let Some(ref a) = author_filter {
            if c.author.to_lowercase() != a.to_lowercase() { return false; }
        }
        if let Some(ref t) = time_after {
            if c.time < *t { return false; }
        }
        if let Some(ref t) = time_before {
            if c.time > *t { return false; }
        }
        if let Some(ref m) = message_contains {
            if !c.message.to_lowercase().contains(&m.to_lowercase()) { return false; }
        }
        true
    }).collect();

    let mut result_commits = filtered;
    if let Some(oi) = order_idx {
        let order_part = &sql[oi + 8..limit_idx.unwrap_or(sql.len())].trim();
        if order_part.to_uppercase().contains("DESC") {
            result_commits.reverse();
        }
    }

    if let Some(li) = limit_idx {
        let limit_part = &sql[li + 5..].trim();
        if let Ok(limit) = limit_part.parse::<usize>() {
            result_commits.truncate(limit);
        }
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    for c in &result_commits {
        let mut row = Vec::new();
        for col in &columns {
            match col.as_str() {
                "hash" => row.push(c.hash.clone()),
                "author" => row.push(c.author.clone()),
                "time" => row.push(c.time.clone()),
                "message" => row.push(c.message.clone()),
                _ => row.push("未知列".into()),
            }
        }
        rows.push(row);
    }

    let elapsed = start.elapsed().as_millis() as u64;
    Ok(QueryResult {
        columns,
        rows,
        elapsed_ms: elapsed,
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_commits,
            get_branches,
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
            git_query
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
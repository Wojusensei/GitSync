#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::{Oid, Repository, Sort};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize)]
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
            execute_rebase
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
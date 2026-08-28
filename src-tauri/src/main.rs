#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::{Oid, Repository, Sort};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::DialogExt;

// 校验来自前端的相对路径：拒绝绝对路径、`..` 等非普通组件，防止目录穿越。
// 返回仅含普通组件的相对路径，可安全 join 到受信目录下。
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

// 在 base 下解析一个必须已存在的文件：canonicalize 后确保没有逃出 base（防符号链接穿越）。
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

        // 上限针对处理的提交数（与 get_hot_files 一致）；原实现误用贡献者人数，
        // 作者不足 50 人的仓库会遍历全部历史并逐提交做 diff，大仓库会长时间卡死
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
    let relative = safe_relative_path(&script_name)?;
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

// ====================
// SQL 查询引擎 v2 — 微型数据库
// ====================

#[derive(Serialize, Debug)]
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

        if commits.len() >= 200 { break; }
    }

    Ok((commits, files))
}

// 旧方言的 CONTAINS 改写为 SQLite 的 LIKE '%x%'（值中的 % _ \ 做转义）。
// 字符串字面量内部不改写；CONTAINS 后面不是字符串字面量时原样保留，交给 SQLite 报语法错误。
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
                    while k < chars.len() && chars[k] != '\'' {
                        val.push(chars[k]);
                        k += 1;
                    }
                    if k < chars.len() {
                        let escaped = val
                            .replace('\\', "\\\\")
                            .replace('%', "\\%")
                            .replace('_', "\\_");
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

fn format_sql_error(e: &rusqlite::Error) -> String {
    let msg = e.to_string();
    if msg.contains("no such table") {
        format!("表不存在，可用表: commits, file_changes（{}）", msg)
    } else {
        msg
    }
}

#[tauri::command]
fn git_query(path: String, sql: String) -> Result<QueryResult, String> {
    let start = std::time::Instant::now();

    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded))
        .map_err(|e| format!("无法打开仓库: {}", e))?;

    let (commits, file_changes) = collect_all_data(&repo)?;

    let sql = rewrite_contains(&sql);

    let conn = rusqlite::Connection::open_in_memory()
        .map_err(|e| format!("无法创建查询引擎: {}", e))?;
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

    let mut stmt = conn.prepare(&sql).map_err(|e| format_sql_error(&e))?;
    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows_iter = stmt.query(rusqlite::params![]).map_err(|e| format_sql_error(&e))?;
    let mut result_rows: Vec<Vec<String>> = Vec::new();
    while let Some(row) = rows_iter.next().map_err(|e| format_sql_error(&e))? {
        let mut row_out = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            let cell = match row.get_ref(i).map_err(|e| format_sql_error(&e))? {
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
    let relative = safe_relative_path(&file_path)?;
    let full_path = resolve_existing_under(Path::new(&expanded), &relative)?;
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
        git_query(dir.to_str().unwrap().to_string(), sql.to_string())
    }

    #[test]
    fn file_changes_table_is_queryable() {
        // 回归：旧解析器把非 commits 主表的行全部丢弃，此查询永远返回 0 行
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT COUNT(*) FROM file_changes").unwrap();
        assert_eq!(res.rows, vec![vec!["4".to_string()]], "两笔提交各改两个文件");
    }

    #[test]
    fn per_file_stats_are_not_whole_commit_totals() {
        // 回归：旧实现把整笔提交的总量（+2）写到每个文件行上
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
        // 回归：旧实现按字符串比较，"9" > "10" 为真
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT COUNT(*) FROM file_changes WHERE additions > 1").unwrap();
        assert_eq!(res.rows, vec![vec!["0".to_string()]]);
        let res = query(dir.path(), "SELECT COUNT(*) FROM file_changes WHERE additions > 0").unwrap();
        assert_eq!(res.rows, vec![vec!["4".to_string()]]);
    }

    #[test]
    fn limit_with_trailing_semicolon() {
        // 回归：旧实现的 LIMIT 解析遇到分号会静默失效
        let dir = tempfile::tempdir().unwrap();
        setup_repo(dir.path());
        let res = query(dir.path(), "SELECT hash FROM commits LIMIT 1;").unwrap();
        assert_eq!(res.rows.len(), 1);
    }

    #[test]
    fn malformed_sql_returns_error_instead_of_panicking() {
        // 回归：旧解析器遇到关键词乱序会切片越界 panic，panic=abort 直接崩掉整个应用
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
        // 回归：超出 chrono 范围的时间 -> "未知时间"（多字节），旧实现切片 [..10] 会 panic。
        // git2 创建提交时会把时间截断成 32 位，因此直接写原始 commit 对象绕过创建路径。
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
        assert!(run_script(repo_str.clone(), "../../usr/bin/env".to_string()).is_err());
        assert!(run_script(repo_str, "/bin/sh".to_string()).is_err());
    }
}

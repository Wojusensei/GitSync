// ====================
// 预导入 Tauri 宏
// ====================

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::Repository;
use serde::Serialize;
use std::path::Path;

// ====================
// 提交数据结构
// ====================

#[derive(Serialize)]
struct Commit {
    hash: String,
    author: String,
    time: String,
    message: String,
}

// ====================
// 读取提交历史
// ====================

#[tauri::command]
fn get_commits(path: String) -> Result<Vec<Commit>, String> {
    let expanded = shellexpand::tilde(&path).to_string();
    let repo = Repository::open(Path::new(&expanded)).map_err(|e| format!("无法打开仓库: {}", e))?;
    let mut commits = Vec::new();

    let mut revwalk = repo.revwalk().map_err(|e| format!("无法创建 revwalk: {}", e))?;
    revwalk.push_head().map_err(|e| format!("无法推送 HEAD: {}", e))?;
    revwalk.set_sorting(git2::Sort::TIME).map_err(|e| format!("无法设置排序: {}", e))?;

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

// ====================
// 启动入口
// ====================

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_commits])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
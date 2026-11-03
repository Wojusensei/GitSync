import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface InlineChange {
  offset: number;
  length: number;
  kind: string;
}

interface DiffLine {
  origin: string;
  content: string;
  inline_changes: InlineChange[];
}

interface DiffHunk {
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  lines: DiffLine[];
}

interface DiffDetail {
  old_content: string;
  new_content: string;
  hunks: DiffHunk[];
}

// 按字符偏移高亮行内变化（Array.from 与后端 chars 对齐）
function renderInline(content: string, changes: InlineChange[] | undefined, kind: 'insert' | 'delete') {
  const chars = Array.from(content);
  const marked = new Array<boolean>(chars.length).fill(false);
  for (const c of changes || []) {
    if (c.kind !== kind) continue;
    for (let i = c.offset; i < c.offset + c.length && i < marked.length; i++) {
      if (i >= 0) marked[i] = true;
    }
  }
  if (!marked.some(Boolean)) return content;

  const parts: React.ReactNode[] = [];
  let run = '';
  let runMarked = marked[0];
  const flush = (key: number) => {
    if (!run) return;
    parts.push(
      runMarked
        ? <span key={key} className={kind === 'insert' ? 'diff-inline-insert' : 'diff-inline-delete'}>{run}</span>
        : <span key={key}>{run}</span>
    );
    run = '';
  };
  for (let i = 0; i < chars.length; i++) {
    if (marked[i] === runMarked) {
      run += chars[i];
    } else {
      flush(i);
      run = chars[i];
      runMarked = marked[i];
    }
  }
  flush(chars.length);
  return parts;
}

export default function SyntaxHighlight({ repoPath, commitHash }: { repoPath: string; commitHash: string }) {
  const [files, setFiles] = useState<[string, DiffDetail][]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const loadDiff = async () => {
    if (!commitHash) {
      setError('请先在提交列表中选中一个提交');
      return;
    }
    setLoading(true);
    setError('');
    try {
      const res = await invoke<[string, DiffDetail][]>('get_diff_detail', { path: repoPath, commitHash });
      setFiles(res);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>Diff 语法高亮</h3>
      <button className="btn btn-blue" onClick={loadDiff} disabled={loading}>
        {loading ? '加载中...' : '加载 Diff'}
      </button>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      {files.map(([path, detail]) => (
        <div key={path} style={{ marginTop: 12 }}>
          <div className="section-title">{path}</div>
          {detail.hunks.map((hunk, i) => (
            <div key={i} style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
              {hunk.lines.map((line, j) => (
                <div key={j} style={{
                  background: line.origin === '+' ? 'var(--diff-add-bg)' : line.origin === '-' ? 'var(--diff-del-bg)' : 'transparent',
                  color: line.origin === '+' ? 'var(--diff-add-text)' : line.origin === '-' ? 'var(--diff-del-text)' : 'var(--text)',
                  padding: '1px 8px',
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'break-all'
                }}>
                  {line.origin}{' '}
                  {line.origin === '+'
                    ? renderInline(line.content, line.inline_changes, 'insert')
                    : line.origin === '-'
                      ? renderInline(line.content, line.inline_changes, 'delete')
                      : line.content}
                </div>
              ))}
            </div>
          ))}
        </div>
      ))}
    </motion.div>
  );
}

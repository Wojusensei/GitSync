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

// 按后端给的字符偏移把行内变化段高亮出来（Array.from 按码点切，与后端 chars().count() 对齐）
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
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}
      {files.map(([path, detail]) => (
        <div key={path} style={{ marginTop: 12 }}>
          <div className="section-title">{path}</div>
          {detail.hunks.map((hunk, i) => (
            <div key={i} style={{ fontFamily: 'monospace', fontSize: 12 }}>
              {hunk.lines.map((line, j) => (
                <div key={j} style={{
                  background: line.origin === '+' ? 'rgba(76,175,80,0.15)' : line.origin === '-' ? 'rgba(244,67,54,0.15)' : 'transparent',
                  color: line.origin === '+' ? '#81c784' : line.origin === '-' ? '#e57373' : '#c8d6e5',
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
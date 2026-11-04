import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { VscClose } from 'react-icons/vsc';
import { getFileIcon } from '../utils/fileIcons';
import { renderHighlightedLine } from '../utils/diffUtils';

interface FileViewerProps {
  repoPath: string;
  filePath: string;
  onClose: () => void;
}

interface LineHistoryEntry {
  commit_hash: string;
  short_hash: string;
  author: string;
  time: string;
  message: string;
  lines: number[];
  sample: string;
}

export default function FileViewer({ repoPath, filePath, onClose }: FileViewerProps) {
  const [content, setContent] = useState<string>('');
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');
  const [selStart, setSelStart] = useState<number | null>(null);
  const [selEnd, setSelEnd] = useState<number | null>(null);
  const [dragAnchor, setDragAnchor] = useState<number | null>(null);
  const [dragging, setDragging] = useState(false);
  const [lineHistory, setLineHistory] = useState<LineHistoryEntry[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);

  useEffect(() => {
    // 只接受最新一次请求的响应
    let cancelled = false;
    const fetchFileContent = async () => {
      setLoading(true);
      setError('');
      try {
        const res = await invoke<string>('get_file_content', { path: repoPath, filePath });
        if (!cancelled) setContent(res);
      } catch (e: any) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    fetchFileContent();
    return () => { cancelled = true; };
  }, [repoPath, filePath]);

  // 拖选行号时松手结束
  useEffect(() => {
    const up = () => setDragging(false);
    window.addEventListener('mouseup', up);
    return () => window.removeEventListener('mouseup', up);
  }, []);

  // 选中行范围后查行级历史（v1：blame 聚合）
  useEffect(() => {
    if (selStart === null || selEnd === null) {
      setLineHistory([]);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(async () => {
      setHistoryLoading(true);
      try {
        const res = await invoke<LineHistoryEntry[]>('get_line_history', {
          path: repoPath, commitHash: 'HEAD', filePath, startLine: selStart, endLine: selEnd,
        });
        if (!cancelled) setLineHistory(res);
      } catch {
        if (!cancelled) setLineHistory([]);
      } finally {
        if (!cancelled) setHistoryLoading(false);
      }
    }, 150);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [repoPath, filePath, selStart, selEnd]);

  const beginSelect = (lineNo: number) => {
    setSelStart(lineNo);
    setSelEnd(lineNo);
    setDragAnchor(lineNo);
    setDragging(true);
  };

  const extendSelect = (lineNo: number) => {
    if (!dragging || dragAnchor === null) return;
    setSelStart(Math.min(dragAnchor, lineNo));
    setSelEnd(Math.max(dragAnchor, lineNo));
  };

  const inSelection = (lineNo: number) =>
    selStart !== null && selEnd !== null && lineNo >= selStart && lineNo <= selEnd;

  const lines = content.split('\n');

  // 二进制内容简单探测
  const isBinary = content.includes('\u0000') || (content.length > 0 && content.slice(0, 1000).replace(/[\x09\x0A\x0D\x20-\x7E]/g, '').length / Math.min(1000, content.length) > 0.3);

  return (
    <motion.div
      className="analysis-panel"
      initial={{ opacity: 0, x: 20 }}
      animate={{ opacity: 1, x: 0 }}
      style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 400 }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--border)', paddingBottom: 10, marginBottom: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, overflow: 'hidden' }}>
          {getFileIcon(filePath, 18)}
          <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {filePath}
          </span>
        </div>
        <button
          onClick={onClose}
          style={{ background: 'transparent', border: 'none', color: 'var(--text)', cursor: 'pointer', display: 'flex', alignItems: 'center', padding: 4 }}
          title="关闭查看"
        >
          <VscClose size={18} />
        </button>
      </div>

      {selStart !== null && (
        <div className="line-history">
          <div className="line-history-head">
            <span>行历史 · 第 {selStart}{selEnd !== null && selEnd !== selStart ? `–${selEnd}` : ''} 行</span>
            {historyLoading && <span style={{ color: 'var(--text-dim)' }}>查询中…</span>}
            <button
              className="detail-action-btn"
              style={{ marginLeft: 'auto' }}
              onClick={() => { setSelStart(null); setSelEnd(null); }}
            >
              清除
            </button>
          </div>
          {lineHistory.map(e => (
            <div key={e.commit_hash} className="line-history-item">
              <span className="line-history-hash">{e.short_hash}</span>
              <span className="line-history-msg" title={e.message}>{e.message.split('\n')[0]}</span>
              <span className="line-history-meta">{e.author} · {e.time} · 行 {e.lines.join(',')}</span>
            </div>
          ))}
          {!historyLoading && lineHistory.length === 0 && <div className="line-history-item">没有找到相关提交</div>}
        </div>
      )}

      {loading ? (
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--text-dim)', minHeight: 300 }}>
          加载文件中...
        </div>
      ) : error ? (
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--danger)', minHeight: 300 }}>
          {error}
        </div>
      ) : isBinary ? (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', color: 'var(--text-dim)', minHeight: 300 }}>
          <p>此文件可能是二进制文件，无法直接预览。</p>
        </div>
      ) : (
        <div
          style={{
            flex: 1,
            overflow: 'auto',
            maxHeight: 500,
            background: 'var(--code-bg)',
            borderRadius: 8,
            border: '1px solid var(--border-faint)',
            fontFamily: 'var(--font-mono)',
            fontSize: 12,
            lineHeight: 1.5,
            padding: '10px 0'
          }}
        >
          <table style={{ borderCollapse: 'collapse', width: '100%' }}>
            <tbody>
              {lines.map((line, idx) => (
                <tr key={idx} className={`file-view-tr${inSelection(idx + 1) ? ' selected' : ''}`} style={{ verticalAlign: 'top' }}>
                  <td
                    onMouseDown={(e) => { e.preventDefault(); beginSelect(idx + 1); }}
                    onMouseEnter={() => extendSelect(idx + 1)}
                    title="点击或拖选查看这些行的历史"
                    style={{
                      width: 40,
                      textAlign: 'right',
                      paddingRight: 10,
                      userSelect: 'none',
                      color: inSelection(idx + 1) ? 'var(--accent)' : 'var(--text-faint)',
                      borderRight: '1px solid var(--border-faint)',
                      fontSize: 11,
                      cursor: 'pointer'
                    }}
                  >
                    {idx + 1}
                  </td>
                  <td style={{ paddingLeft: 10, whiteSpace: 'pre', color: 'var(--text)' }}>
                    {renderHighlightedLine(line)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </motion.div>
  );
}

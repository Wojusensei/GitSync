import { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

interface ReflogEntry {
  index: number;
  hash: string;
  short_hash: string;
  action: string;
  detail: string;
  committer: string;
  time: string;
  is_current: boolean;
}

// 找回丢失提交：列出 HEAD 的 reflog，允许在任何一条上新建分支
export default function ReflogPanel({ repoPath, onBranchCreated }: {
  repoPath: string;
  onBranchCreated?: () => void;
}) {
  const [entries, setEntries] = useState<ReflogEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [creating, setCreating] = useState<string | null>(null);

  const load = async () => {
    setError('');
    setNotice('');
    setLoading(true);
    try {
      setEntries(await invokeTauri<ReflogEntry[]>('get_reflog', { path: repoPath }));
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, [repoPath]);

  const restore = async (entry: ReflogEntry) => {
    setError('');
    setNotice('');
    setCreating(entry.hash);
    const name = `recover/${entry.short_hash}`;
    try {
      await invokeTauri('restore_reflog_branch', { path: repoPath, commitHash: entry.hash, branchName: name });
      setNotice(`已在该提交上新建分支 ${name}`);
      onBranchCreated?.();
    } catch (e: any) {
      setError(String(e));
    } finally {
      setCreating(null);
    }
  };

  const copyHash = async (entry: ReflogEntry) => {
    try {
      await navigator.clipboard.writeText(entry.hash);
      setNotice(`已复制 ${entry.short_hash}`);
    } catch {
      setError('复制失败');
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <div className="panel-title-row">
        <h3>Reflog 找回</h3>
        <span className="panel-hint">
          {loading ? '加载中…' : `${entries.length} 条记录`}
        </span>
      </div>
      <div style={{ display: 'flex', gap: 10, marginBottom: 12 }}>
        <button className="btn" onClick={load} disabled={loading}>刷新</button>
      </div>
      {notice && <div className="analysis-item" style={{ color: 'var(--success)', fontSize: 12 }}>{notice}</div>}
      {error && <div className="analysis-item" style={{ color: 'var(--danger)', fontSize: 12 }}>{error}</div>}
      {!loading && entries.length === 0 && !error && (
        <div className="analysis-item" style={{ color: 'var(--text-dim)', fontSize: 12 }}>没有 reflog 记录</div>
      )}
      <div className="reflog-list">
        {entries.map(entry => (
          <div key={`${entry.index}-${entry.hash}`} className={`reflog-row${entry.is_current ? ' current' : ''}`}>
            <div className="reflog-main">
              <div className="reflog-line">
                <span className="reflog-action">{entry.action}</span>
                <span className="reflog-hash">{entry.short_hash}</span>
                {entry.is_current && <span className="reflog-current">当前 HEAD</span>}
              </div>
              <div className="reflog-detail" title={entry.detail}>{entry.detail || '（无描述）'}</div>
              <div className="reflog-meta">{entry.committer} · {entry.time}</div>
            </div>
            <div className="reflog-actions">
              <button
                className="btn btn-blue"
                style={{ padding: '4px 10px', fontSize: 12 }}
                onClick={() => restore(entry)}
                disabled={creating !== null}
              >
                {creating === entry.hash ? '创建中…' : '新建分支'}
              </button>
              <button className="btn" style={{ padding: '4px 10px', fontSize: 12 }} onClick={() => copyHash(entry)}>
                复制哈希
              </button>
            </div>
          </div>
        ))}
      </div>
    </motion.div>
  );
}

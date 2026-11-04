import { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

interface SubmoduleInfo {
  name: string;
  path: string;
  url: string | null;
  head_oid: string | null;
  status: string;
}

interface WorktreeInfo {
  name: string;
  path: string;
  branch: string | null;
  head: string | null;
}

export default function RepoExtrasPanel({ repoPath, onOpenRepo }: {
  repoPath: string;
  onOpenRepo: (path: string) => void;
}) {
  const [submodules, setSubmodules] = useState<SubmoduleInfo[]>([]);
  const [worktrees, setWorktrees] = useState<WorktreeInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');

  const load = async () => {
    setError('');
    setLoading(true);
    try {
      const [subs, wts] = await Promise.all([
        invokeTauri<SubmoduleInfo[]>('get_submodules', { path: repoPath }),
        invokeTauri<WorktreeInfo[]>('get_worktrees', { path: repoPath }),
      ]);
      setSubmodules(subs);
      setWorktrees(wts);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, [repoPath]);

  const updateAll = async () => {
    setError('');
    setNotice('');
    setUpdating(true);
    try {
      await invokeTauri('update_submodules', { path: repoPath });
      setNotice('子模块已更新');
      await load();
    } catch (e: any) {
      setError(String(e));
    } finally {
      setUpdating(false);
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <div className="panel-title-row">
        <h3>子模块与 Worktree</h3>
        <span className="panel-hint">{loading ? '加载中…' : `${submodules.length} 个子模块 · ${worktrees.length} 个 worktree`}</span>
      </div>
      <div style={{ display: 'flex', gap: 10, marginBottom: 12 }}>
        <button className="btn" onClick={load} disabled={loading}>刷新</button>
        <button className="btn btn-blue" onClick={updateAll} disabled={updating || submodules.length === 0}>
          {updating ? '更新中…' : '更新子模块'}
        </button>
      </div>
      {notice && <div className="analysis-item" style={{ color: 'var(--success)', fontSize: 12 }}>{notice}</div>}
      {error && <div className="analysis-item" style={{ color: 'var(--danger)', fontSize: 12 }}>{error}</div>}

      <div className="analysis-section">
        <div className="section-title">子模块</div>
        {submodules.length === 0 && <div className="extras-empty">这个仓库没有子模块</div>}
        {submodules.map(sm => (
          <div key={sm.path} className="extras-row">
            <div className="extras-main">
              <div className="extras-title">{sm.name}<span className="extras-path">{sm.path}</span></div>
              <div className="extras-meta">{sm.url || '无 URL'}{sm.head_oid ? ` · ${sm.head_oid.substring(0, 8)}` : ''}</div>
            </div>
            <span className={`extras-badge${sm.status === '正常' ? '' : ' warn'}`}>{sm.status}</span>
          </div>
        ))}
      </div>

      <div className="analysis-section">
        <div className="section-title">Worktree</div>
        {worktrees.length === 0 && <div className="extras-empty">没有额外的 worktree</div>}
        {worktrees.map(wt => (
          <div key={wt.name} className="extras-row">
            <div className="extras-main">
              <div className="extras-title">{wt.name}<span className="extras-path">{wt.branch || '（游离 HEAD）'}</span></div>
              <div className="extras-meta">{wt.path}{wt.head ? ` · ${wt.head.substring(0, 8)}` : ''}</div>
            </div>
            <button className="btn" style={{ padding: '4px 10px', fontSize: 12 }} onClick={() => onOpenRepo(wt.path)}>切换查看</button>
          </div>
        ))}
      </div>
    </motion.div>
  );
}

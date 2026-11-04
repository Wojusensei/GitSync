import { useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

interface ConflictBlock {
  ours_text: string;
  theirs_text: string;
}

interface ConflictDetail {
  path: string;
  ours: string;
  theirs: string;
  merged: string;
  conflict_blocks: ConflictBlock[];
}

export default function ConflictResolver({ repoPath }: { repoPath: string }) {
  const [files, setFiles] = useState<ConflictDetail[]>([]);
  const [activeFile, setActiveFile] = useState<ConflictDetail | null>(null);
  const [resolutions, setResolutions] = useState<string[]>([]);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [notice, setNotice] = useState('');

  const load = async () => {
    setError('');
    setNotice('');
    setLoading(true);
    try {
      const res = await invokeTauri<ConflictDetail[]>('get_conflict_detail', { path: repoPath });
      setFiles(res);
      if (res.length > 0) {
        setActiveFile(res[0]);
        setResolutions(res[0].conflict_blocks.map(() => ''));
        setNotice(`已加载 ${res.length} 个冲突文件`);
      } else {
        setActiveFile(null);
        setResolutions([]);
        setNotice('当前没有冲突文件');
      }
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const applyOurs = (index: number) => {
    if (!activeFile) return;
    const newResolutions = [...resolutions];
    newResolutions[index] = activeFile.conflict_blocks[index].ours_text;
    setResolutions(newResolutions);
  };

  const applyTheirs = (index: number) => {
    if (!activeFile) return;
    const newResolutions = [...resolutions];
    newResolutions[index] = activeFile.conflict_blocks[index].theirs_text;
    setResolutions(newResolutions);
  };

  const applyAllOurs = () => {
    if (!activeFile) return;
    setResolutions(activeFile.conflict_blocks.map(b => b.ours_text));
  };

  const applyAllTheirs = () => {
    if (!activeFile) return;
    setResolutions(activeFile.conflict_blocks.map(b => b.theirs_text));
  };

  const save = async () => {
    if (!activeFile) return;
    setError('');
    try {
      // 后端负责把每块的解决文本插回冲突原位置，并 git add 标记已解决
      await invokeTauri('resolve_conflict', { path: repoPath, filePath: activeFile.path, resolutions });
      setFiles(files.filter(f => f.path !== activeFile.path));
      if (files.length > 1) {
        const remaining = files.filter(f => f.path !== activeFile.path);
        setActiveFile(remaining[0]);
        setResolutions(remaining[0].conflict_blocks.map(() => ''));
      } else {
        setActiveFile(null);
        setResolutions([]);
      }
    } catch (e: any) {
      setError(String(e));
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <h3>图形化冲突解决</h3>
      <button className="btn btn-blue" onClick={load} disabled={loading}>
        {loading ? '加载中…' : '加载冲突文件'}
      </button>
      {notice && <div className="analysis-item" style={{ color: 'var(--text-dim)', fontSize: 12 }}>{notice}</div>}
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}

      {files.length > 0 && (
        <div style={{ display: 'flex', gap: 8, marginTop: 12, flexWrap: 'wrap' }}>
          {files.map(f => (
            <button
              key={f.path}
              className={`btn ${activeFile?.path === f.path ? 'btn-blue' : ''}`}
              onClick={() => { setActiveFile(f); setResolutions(f.conflict_blocks.map(() => '')); }}
              style={{ fontSize: 12, padding: '4px 12px' }}
            >
              {f.path}
            </button>
          ))}
        </div>
      )}

      {activeFile && (
        <div style={{ marginTop: 12 }}>
          <div className="section-title" style={{ marginBottom: 8 }}>{activeFile.path}</div>
          {activeFile.conflict_blocks.map((block, i) => (
            <div key={i} style={{ marginBottom: 16, border: '1px solid var(--border)', borderRadius: 10, overflow: 'hidden' }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 1, background: 'var(--panel)' }}>
                <div style={{ padding: 8, fontSize: 12, color: 'var(--text-dim)', borderBottom: '1px solid var(--border-faint)' }}>当前分支 (ours)</div>
                <div style={{ padding: 8, fontSize: 12, color: 'var(--text-dim)', borderBottom: '1px solid var(--border-faint)' }}>合并结果</div>
                <div style={{ padding: 8, fontSize: 12, color: 'var(--text-dim)', borderBottom: '1px solid var(--border-faint)' }}>传入分支 (theirs)</div>

                <div style={{ background: 'var(--danger-dim)', padding: 10, fontFamily: 'var(--font-mono)', fontSize: 12, whiteSpace: 'pre-wrap' }}>
                  {block.ours_text}
                  <button className="btn btn-blue" onClick={() => applyOurs(i)} style={{ display: 'block', marginTop: 8, width: '100%', fontSize: 11, padding: '4px 8px' }}>采用此边 →</button>
                </div>

                <div style={{ background: 'var(--panel)', padding: 10, fontFamily: 'var(--font-mono)', fontSize: 12, whiteSpace: 'pre-wrap' }}>
                  <textarea
                    value={resolutions[i] || ''}
                    onChange={e => { const newRes = [...resolutions]; newRes[i] = e.target.value; setResolutions(newRes); }}
                    style={{ width: '100%', minHeight: 80, background: 'transparent', color: 'var(--text)', border: '1px solid var(--border-strong)', borderRadius: 6, padding: 6, fontFamily: 'inherit', fontSize: 'inherit', resize: 'vertical' }}
                  />
                </div>

                <div style={{ background: 'var(--success-dim)', padding: 10, fontFamily: 'var(--font-mono)', fontSize: 12, whiteSpace: 'pre-wrap' }}>
                  {block.theirs_text}
                  <button className="btn btn-blue" onClick={() => applyTheirs(i)} style={{ display: 'block', marginTop: 8, width: '100%', fontSize: 11, padding: '4px 8px' }}>采用此边 →</button>
                </div>
              </div>
            </div>
          ))}

          <div style={{ display: 'flex', gap: 8, marginTop: 12, flexWrap: 'wrap' }}>
            <button className="btn" onClick={applyAllOurs} style={{ background: 'var(--danger-soft)', color: 'var(--danger)', border: '1px solid var(--danger-border)', borderRadius: 8, padding: '6px 14px', fontSize: 12, cursor: 'pointer' }}>全部采用当前分支</button>
            <button className="btn" onClick={applyAllTheirs} style={{ background: 'var(--success-dim)', color: 'var(--success)', border: '1px solid var(--success-border)', borderRadius: 8, padding: '6px 14px', fontSize: 12, cursor: 'pointer' }}>全部采用传入分支</button>
            <button className="btn btn-blue" onClick={save} style={{ marginLeft: 'auto' }}>保存并标记已解决</button>
          </div>
        </div>
      )}
    </motion.div>
  );
}

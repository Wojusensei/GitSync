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

  const load = async () => {
    setError('');
    try {
      const res = await invokeTauri<ConflictDetail[]>('get_conflict_detail', { path: repoPath });
      setFiles(res);
      if (res.length > 0) {
        setActiveFile(res[0]);
        setResolutions(res[0].conflict_blocks.map(() => ''));
      }
    } catch (e: any) {
      setError(String(e));
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

  const buildResolution = (): string => {
    if (!activeFile) return '';
    let result = activeFile.merged;
    for (let i = 0; i < activeFile.conflict_blocks.length; i++) {
      result += resolutions[i] || activeFile.conflict_blocks[i].ours_text;
    }
    return result;
  };

  const save = async () => {
    if (!activeFile) return;
    setError('');
    try {
      await invokeTauri('resolve_conflict', { path: repoPath, filePath: activeFile.path, resolution: buildResolution() });
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
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>图形化冲突解决</h3>
      <button className="btn btn-blue" onClick={load}>加载冲突文件</button>
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}

      {files.length > 0 && (
        <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
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
            <div key={i} style={{ marginBottom: 16, border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, overflow: 'hidden' }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 1, background: 'rgba(255,255,255,0.02)' }}>
                <div style={{ padding: 8, fontSize: 12, color: '#8899aa', borderBottom: '1px solid rgba(255,255,255,0.04)' }}>当前分支 (ours)</div>
                <div style={{ padding: 8, fontSize: 12, color: '#8899aa', borderBottom: '1px solid rgba(255,255,255,0.04)' }}>合并结果</div>
                <div style={{ padding: 8, fontSize: 12, color: '#8899aa', borderBottom: '1px solid rgba(255,255,255,0.04)' }}>传入分支 (theirs)</div>

                <div style={{ background: 'rgba(244,67,54,0.08)', padding: 10, fontFamily: 'monospace', fontSize: 12, whiteSpace: 'pre-wrap' }}>
                  {block.ours_text}
                  <button className="btn btn-blue" onClick={() => applyOurs(i)} style={{ display: 'block', marginTop: 8, width: '100%', fontSize: 11, padding: '4px 8px' }}>采用此边 →</button>
                </div>

                <div style={{ background: 'rgba(255,255,255,0.03)', padding: 10, fontFamily: 'monospace', fontSize: 12, whiteSpace: 'pre-wrap' }}>
                  <textarea
                    value={resolutions[i] || ''}
                    onChange={e => { const newRes = [...resolutions]; newRes[i] = e.target.value; setResolutions(newRes); }}
                    style={{ width: '100%', minHeight: 80, background: 'transparent', color: '#c8d6e5', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 6, padding: 6, fontFamily: 'inherit', fontSize: 'inherit', resize: 'vertical' }}
                  />
                </div>

                <div style={{ background: 'rgba(76,175,80,0.08)', padding: 10, fontFamily: 'monospace', fontSize: 12, whiteSpace: 'pre-wrap' }}>
                  {block.theirs_text}
                  <button className="btn btn-blue" onClick={() => applyTheirs(i)} style={{ display: 'block', marginTop: 8, width: '100%', fontSize: 11, padding: '4px 8px' }}>采用此边 →</button>
                </div>
              </div>
            </div>
          ))}

          <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
            <button className="btn" onClick={applyAllOurs} style={{ background: 'rgba(244,67,54,0.15)', color: '#ff6b6b', border: '1px solid rgba(244,67,54,0.2)', borderRadius: 8, padding: '6px 14px', fontSize: 12, cursor: 'pointer' }}>全部采用当前分支</button>
            <button className="btn" onClick={applyAllTheirs} style={{ background: 'rgba(76,175,80,0.15)', color: '#4caf50', border: '1px solid rgba(76,175,80,0.2)', borderRadius: 8, padding: '6px 14px', fontSize: 12, cursor: 'pointer' }}>全部采用传入分支</button>
            <button className="btn btn-blue" onClick={save} style={{ marginLeft: 'auto' }}>保存并标记已解决</button>
          </div>
        </div>
      )}
    </motion.div>
  );
}
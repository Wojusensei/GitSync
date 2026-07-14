import { useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

export default function HookManager({ repoPath }: { repoPath: string }) {
  const [hooks, setHooks] = useState<string[]>([]);
  const [selectedHook, setSelectedHook] = useState('');
  const [content, setContent] = useState('');
  const [error, setError] = useState('');

  const loadHooks = async () => {
    setError('');
    try {
      const res = await invokeTauri<string[]>('get_hooks', { path: repoPath });
      setHooks(res);
    } catch (e: any) {
      setError(String(e));
    }
  };

  const loadContent = async (name: string) => {
    setError('');
    try {
      const res = await invokeTauri<string>('get_hook_content', { path: repoPath, hookName: name });
      setSelectedHook(name);
      setContent(res);
    } catch (e: any) {
      setError(String(e));
    }
  };

  const saveContent = async () => {
    setError('');
    try {
      await invokeTauri('save_hook_content', { path: repoPath, hookName: selectedHook, content });
      alert('保存成功');
    } catch (e: any) {
      setError(String(e));
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>Git Hooks 管理器</h3>
      <button className="btn btn-blue" onClick={loadHooks}>加载 Hooks</button>
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}
      {hooks.map(h => (
        <div key={h} className="analysis-item" onClick={() => loadContent(h)} style={{ cursor: 'pointer' }}>
          {h}
        </div>
      ))}
      {selectedHook && (
        <div style={{ marginTop: 12 }}>
          <div className="section-title">编辑: {selectedHook}</div>
          <textarea className="path-input" value={content} onChange={e => setContent(e.target.value)} style={{ minHeight: 200, fontFamily: 'monospace' }} />
          <button className="btn btn-blue" onClick={saveContent}>保存</button>
        </div>
      )}
    </motion.div>
  );
}
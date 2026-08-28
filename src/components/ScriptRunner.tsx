import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

export default function ScriptRunner({ repoPath }: { repoPath: string }) {
  const [scripts, setScripts] = useState<string[]>([]);
  const [output, setOutput] = useState('');
  const [error, setError] = useState('');
  const [running, setRunning] = useState(false);

  const loadScripts = async () => {
    setError('');
    try {
      const res = await invoke<string[]>('list_scripts');
      setScripts(res);
    } catch (e: any) {
      setError(String(e));
    }
  };

  const runScript = async (name: string) => {
    setError('');
    setOutput('');
    setRunning(true);
    try {
      const res = await invoke<string>('run_script', { path: repoPath, scriptName: name });
      setOutput(res);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>脚本扩展</h3>
      <button className="btn btn-blue" onClick={loadScripts}>加载脚本</button>
      <p style={{ fontSize: 12, color: 'var(--text-dim)', marginTop: 8 }}>
        将脚本放入 ~/.git-tool/scripts 目录后点击“加载脚本”
      </p>
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}
      {scripts.map(s => (
        <div key={s} className="analysis-item" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <span>{s}</span>
          <button className="btn btn-blue" onClick={() => runScript(s)} disabled={running} style={{ padding: '4px 12px', fontSize: 12 }}>
            {running ? '执行中...' : '运行'}
          </button>
        </div>
      ))}
      {output && (
        <div style={{ marginTop: 12 }}>
          <div className="section-title">输出</div>
          <pre style={{ background: 'rgba(255,255,255,0.03)', padding: 12, borderRadius: 8, fontSize: 12, whiteSpace: 'pre-wrap', maxHeight: 400, overflow: 'auto' }}>
            {output}
          </pre>
        </div>
      )}
    </motion.div>
  );
}
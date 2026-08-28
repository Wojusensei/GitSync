import { useState } from 'react';
import { motion, Reorder } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

interface RebaseCommit {
  hash: string;
  message: string;
  author: string;
  time: string;
}

interface RebaseOperation {
  hash: string;
  action: string;
  message: string;
  new_message?: string;
}

export default function EnhancedRebase({ repoPath, onComplete }: { repoPath: string; onComplete: () => void }) {

  const [ops, setOps] = useState<RebaseOperation[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const load = async () => {
    setError('');
    try {
      const res = await invokeTauri<RebaseCommit[]>('get_rebase_commits', { path: repoPath, count: 20 });
      setOps(res.map(c => ({ hash: c.hash, action: 'pick', message: c.message })));
    } catch (e) {
      setError(String(e));
    }
  };

  // 不可变更新：直接改 ops 里的对象会绕过 React 的状态比对
  const updateOp = (idx: number, patch: Partial<RebaseOperation>) => {
    setOps(prev => prev.map((op, i) => (i === idx ? { ...op, ...patch } : op)));
  };

  const execute = async () => {
    if (ops.length === 0) {
      setError('请先加载提交');
      return;
    }
    // rebase 会重写最近 N 个提交，属破坏性操作，先确认
    const dropCount = ops.filter(o => o.action === 'drop').length;
    const summary = dropCount > 0 ? `（其中 ${dropCount} 个将被丢弃）` : '';
    if (!confirm(`将改写最近 ${ops.length} 个提交${summary}，确定继续？`)) return;

    setLoading(true);
    setError('');
    try {
      // reword 输入被清空时不下发空 new_message，后端会回退到原提交信息
      const payload = ops.map(op => ({
        ...op,
        new_message: op.action === 'reword' && op.new_message?.trim() ? op.new_message : undefined,
      }));
      await invokeTauri('execute_rebase', { path: repoPath, operations: payload });
      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>交互 Rebase 增强</h3>
      <button className="btn btn-blue" onClick={load} style={{ marginBottom: 12 }}>加载提交</button>
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}
      <Reorder.Group values={ops} onReorder={setOps}>
        {ops.map((op, idx) => (
          <Reorder.Item key={op.hash} value={op}>
            <div className="rebase-item">
              <span className="hash">{op.hash.substring(0, 8)}</span>
              <select value={op.action} onChange={(e) => updateOp(idx, { action: e.target.value })}>
                <option value="pick">pick</option>
                <option value="squash">squash</option>
                <option value="drop">drop</option>
                <option value="reword">reword</option>
              </select>
              {op.action === 'reword' && (
                <input
                  className="rebase-message-input"
                  value={op.new_message ?? op.message}
                  placeholder={op.message}
                  onChange={(e) => updateOp(idx, { new_message: e.target.value })}
                />
              )}
            </div>
          </Reorder.Item>
        ))}
      </Reorder.Group>
      {ops.length > 0 && (
        <button className="btn btn-blue" onClick={execute} disabled={loading} style={{ marginTop: 12 }}>
          {loading ? '执行中...' : '执行 Rebase'}
        </button>
      )}
    </motion.div>
  );
}

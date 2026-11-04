import { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

const MAX_DIFF_LINES = 3000;

interface RefOption {
  group: string;
  value: string;
  label: string;
}

export default function DiffViewer({ repoPath }: { repoPath: string }) {
  const [commitA, setCommitA] = useState('');
  const [commitB, setCommitB] = useState('');
  const [diff, setDiff] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [refs, setRefs] = useState<RefOption[]>([]);

  // 分支 / 标签 / stash 都能直接选，选区只是替用户填输入框
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [branches, tags, stashes] = await Promise.all([
          invokeTauri<any[]>('get_branches', { path: repoPath }),
          invokeTauri<any[]>('get_tags', { path: repoPath }),
          invokeTauri<any[]>('stash_list', { path: repoPath }),
        ]);
        const options: RefOption[] = [
          ...branches.map(b => ({ group: '分支', value: b.name, label: `${b.name}${b.is_head ? '（HEAD）' : ''}` })),
          ...tags.map(t => ({ group: '标签', value: t.name, label: t.name })),
          ...stashes.map(s => ({ group: 'Stash', value: `stash@{${s.index}}`, label: `stash@{${s.index}} ${s.message}` })),
        ];
        if (!cancelled) setRefs(options);
      } catch {
        // 拉不到 ref 列表时仍可手输哈希
      }
    })();
    return () => { cancelled = true; };
  }, [repoPath]);

  const refGroups = Array.from(new Set(refs.map(r => r.group)));

  const handleCompare = async () => {
    setLoading(true);
    setError('');
    setDiff('');
    try {
      const res: any = await invokeTauri('compare_commits', { path: repoPath, commitA, commitB });
      setDiff(res.diff);
    } catch (e: any) {
      setError(String(e));
    } finally { setLoading(false); }
  };

  const swap = () => {
    setCommitA(commitB);
    setCommitB(commitA);
  };

  const lines = diff ? diff.split('\n') : [];
  const shown = lines.slice(0, MAX_DIFF_LINES);
  const added = lines.filter(l => l.startsWith('+') && !l.startsWith('+++')).length;
  const removed = lines.filter(l => l.startsWith('-') && !l.startsWith('---')).length;

  const lineClass = (line: string) => {
    if (line.startsWith('@@')) return 'diff-line hunk';
    if (line.startsWith('+++') || line.startsWith('---')) return 'diff-line meta';
    if (line.startsWith('diff --git') || line.startsWith('index ')) return 'diff-line meta';
    if (line.startsWith('+')) return 'diff-line add';
    if (line.startsWith('-')) return 'diff-line del';
    return 'diff-line';
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <div className="panel-title-row">
        <h3>可视化差异对比</h3>
        {diff && (
          <span className="panel-hint">
            <span style={{ color: 'var(--diff-add-text)' }}>+{added}</span>
            {' / '}
            <span style={{ color: 'var(--diff-del-text)' }}>-{removed}</span>
          </span>
        )}
      </div>
      <div className="diff-compare-bar">
        <label className="filter-field">
          <span>提交 A（旧）</span>
          <div className="diff-ref-row">
            <select
              className="filter-input"
              value={refs.some(r => r.value === commitA) ? commitA : ''}
              onChange={e => e.target.value && setCommitA(e.target.value)}
            >
              <option value="">分支 / 标签 / Stash</option>
              {refGroups.map(group => (
                <optgroup key={group} label={group}>
                  {refs.filter(r => r.group === group).map(r => (
                    <option key={`${group}-${r.value}`} value={r.value}>{r.label}</option>
                  ))}
                </optgroup>
              ))}
            </select>
            <input className="filter-input" value={commitA} onChange={e => setCommitA(e.target.value)} placeholder="或输入哈希 / ref" />
          </div>
        </label>
        <button className="diff-swap" onClick={swap} title="交换 A / B" type="button">⇄</button>
        <label className="filter-field">
          <span>提交 B（新）</span>
          <div className="diff-ref-row">
            <select
              className="filter-input"
              value={refs.some(r => r.value === commitB) ? commitB : ''}
              onChange={e => e.target.value && setCommitB(e.target.value)}
            >
              <option value="">分支 / 标签 / Stash</option>
              {refGroups.map(group => (
                <optgroup key={group} label={group}>
                  {refs.filter(r => r.group === group).map(r => (
                    <option key={`${group}-${r.value}`} value={r.value}>{r.label}</option>
                  ))}
                </optgroup>
              ))}
            </select>
            <input className="filter-input" value={commitB} onChange={e => setCommitB(e.target.value)} placeholder="或输入哈希 / ref" />
          </div>
        </label>
        <button className="btn btn-blue" onClick={handleCompare} disabled={loading || !commitA.trim() || !commitB.trim()}>
          {loading ? '对比中…' : '对比'}
        </button>
      </div>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      {diff && (
        <div className="diff-view">
          {shown.map((line, i) => (
            <div key={i} className={lineClass(line)}>{line || ' '}</div>
          ))}
          {lines.length > MAX_DIFF_LINES && (
            <div className="diff-line meta">… 只显示前 {MAX_DIFF_LINES} 行，共 {lines.length} 行</div>
          )}
        </div>
      )}
    </motion.div>
  );
}

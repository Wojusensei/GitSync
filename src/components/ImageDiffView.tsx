import { useEffect, useState } from 'react';
import { invokeTauri } from '../services/tauriService';

interface ImageDiff {
  old_mime: string | null;
  old_b64: string | null;
  new_mime: string | null;
  new_b64: string | null;
}

export const IMAGE_DIFF_RE = /\.(png|jpe?g|gif|webp|bmp|svg)$/i;

const kb = (b64: string | null) => b64 ? `${(b64.length * 0.75 / 1024).toFixed(1)} KB` : '—';

export default function ImageDiffView({ repoPath, commitHash, filePath }: {
  repoPath: string;
  commitHash: string;
  filePath: string;
}) {
  const [data, setData] = useState<ImageDiff | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await invokeTauri<ImageDiff>('get_image_diff', { path: repoPath, commitHash, filePath });
        if (!cancelled) setData(res);
      } catch (e: any) {
        if (!cancelled) setError(String(e));
      }
    })();
    return () => { cancelled = true; };
  }, [repoPath, commitHash, filePath]);

  const pane = (label: string, mime: string | null, b64: string | null) => (
    <div className="image-diff-pane">
      <div className="image-diff-label">
        <span>{label}</span>
        <span className="image-diff-size">{b64 ? kb(b64) : '不存在'}</span>
      </div>
      {mime && b64 ? (
        <img src={`data:${mime};base64,${b64}`} alt={`${filePath} ${label}`} />
      ) : (
        <div className="image-diff-empty">{label === '旧' ? '此版本没有该图片' : '图片已删除'}</div>
      )}
    </div>
  );

  if (error) return <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>;
  if (!data) return <div className="analysis-item" style={{ color: 'var(--text-dim)' }}>加载图片中…</div>;

  return (
    <div className="image-diff">
      {pane('旧', data.old_mime, data.old_b64)}
      {pane('新', data.new_mime, data.new_b64)}
    </div>
  );
}

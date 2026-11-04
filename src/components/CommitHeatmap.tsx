import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface CalendarDay {
  date: string;
  count: number;
}

const CELL = 12;
const GAP = 3;
const WEEKS = 53;

// 0-4 级色阶，值越大颜色越深
function level(count: number) {
  if (count <= 0) return 0;
  if (count <= 2) return 1;
  if (count <= 5) return 2;
  if (count <= 9) return 3;
  return 4;
}

function fmt(d: Date) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export default function CommitHeatmap({ repoPath }: { repoPath: string }) {
  const [days, setDays] = useState<CalendarDay[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError('');
    invoke<CalendarDay[]>('get_commit_calendar', { path: repoPath, days: 365 })
      .then(res => { if (!cancelled) setDays(res); })
      .catch(e => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [repoPath]);

  const { cells, months, total } = useMemo(() => {
    const counts = new Map(days.map(d => [d.date, d.count]));
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    // 从今天倒推 53 周，并对齐到周日起始
    const start = new Date(today);
    start.setDate(start.getDate() - (WEEKS - 1) * 7 - today.getDay());

    const cells: { date: string; count: number; week: number; dow: number }[] = [];
    const months: { label: string; week: number }[] = [];
    let lastMonth = -1;
    let total = 0;
    for (let i = 0; ; i++) {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      if (d > today) break;
      const date = fmt(d);
      const count = counts.get(date) ?? 0;
      total += count;
      const week = Math.floor(i / 7);
      const dow = d.getDay();
      cells.push({ date, count, week, dow });
      if (dow === 0 && d.getMonth() !== lastMonth) {
        lastMonth = d.getMonth();
        months.push({ label: `${d.getMonth() + 1}月`, week });
      }
    }
    return { cells, months, total };
  }, [days]);

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <h3>提交热力图</h3>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      {loading ? (
        <div className="analysis-item" style={{ color: 'var(--text-dim)' }}>加载中...</div>
      ) : (
        <>
          <div style={{ fontSize: 12, color: 'var(--text-dim)', marginBottom: 8 }}>
            过去一年 {total} 个提交
          </div>
          <div style={{ overflowX: 'auto', paddingBottom: 4 }}>
            <div style={{ display: 'grid', gridTemplateColumns: `repeat(${WEEKS}, ${CELL}px)`, gap: 3, marginLeft: 18 }}>
              {months.map(m => (
                <span
                  key={`${m.label}-${m.week}`}
                  style={{ gridColumn: m.week + 1, fontSize: 10, color: 'var(--text-faint)', whiteSpace: 'nowrap' }}
                >
                  {m.label}
                </span>
              ))}
            </div>
            <div style={{ display: 'grid', gridAutoFlow: 'column', gridTemplateRows: `repeat(7, ${CELL}px)`, gridAutoColumns: `${CELL}px`, gap: GAP }}>
              {cells.map(c => (
                <div
                  key={c.date}
                  className={`hm hm-${level(c.count)}`}
                  style={{ gridRow: c.dow + 1, gridColumn: c.week + 1 }}
                  title={`${c.date}：${c.count} 个提交`}
                />
              ))}
            </div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginTop: 8, fontSize: 10, color: 'var(--text-faint)' }}>
            少
            {[0, 1, 2, 3, 4].map(l => <span key={l} className={`hm hm-${l}`} style={{ display: 'inline-block' }} />)}
            多
          </div>
        </>
      )}
    </motion.div>
  );
}

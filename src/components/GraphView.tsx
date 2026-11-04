import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface GraphCommit {
  hash: string;
  author: string;
  time: string;
  message: string;
  parent_hashes: string[];
}

interface GraphRow {
  commit: GraphCommit;
  lane: number;
  x: number;
  y: number;
}

const ROW_HEIGHT = 40;
const LANE_WIDTH = 16;
const LANE_LEFT = 26;
// 每条泳道一个颜色，挨着的色相拉开，合并线一眼能分清
const PALETTE = ['#4a9eff', '#e05fb0', '#34c98e', '#d9a72e', '#e0703c', '#a970e0', '#38bdf8', '#c0d13c'];

export default function GraphView({ repoPath, onSelectCommit }: { repoPath: string; onSelectCommit: (hash: string) => void }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const positionsRef = useRef<Record<string, { x: number; y: number }>>({});
  const [error, setError] = useState('');
  // ref 持有最新回调，画布只在 repoPath 变化时重绘
  const onSelectRef = useRef(onSelectCommit);
  useEffect(() => {
    onSelectRef.current = onSelectCommit;
  });

  useEffect(() => {
    const loadGraph = async () => {
      setError('');
      try {
        const commits = await invoke<GraphCommit[]>('get_graph_commits', { path: repoPath });
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const css = getComputedStyle(document.documentElement);
        const bg = css.getPropertyValue('--bg').trim() || '#0a0a14';
        const textColor = css.getPropertyValue('--text-secondary').trim() || '#c8d6e5';
        const faintColor = css.getPropertyValue('--text-faint').trim() || 'rgba(233,238,247,0.35)';

        const width = containerRef.current?.clientWidth || 800;
        const height = Math.max(commits.length * ROW_HEIGHT, ROW_HEIGHT * 2);
        const dpr = window.devicePixelRatio || 1;
        canvas.width = Math.round(width * dpr);
        canvas.height = Math.round(height * dpr);
        canvas.style.width = `${width}px`;
        canvas.style.height = `${height}px`;
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, width, height);

        // 泳道分配：active[i] 是第 i 条泳道下一行期待的提交
        const active: (string | null)[] = [];
        const findFreeLane = () => {
          const free = active.indexOf(null);
          if (free >= 0) return free;
          active.push(null);
          return active.length - 1;
        };

        const rows: GraphRow[] = [];
        commits.forEach((commit, index) => {
          let lane = active.indexOf(commit.hash);
          if (lane < 0) lane = findFreeLane();
          // 这个提交消费掉所有指向它的期待
          for (let k = 0; k < active.length; k++) {
            if (active[k] === commit.hash) active[k] = null;
          }
          rows.push({ commit, lane, x: LANE_LEFT + lane * LANE_WIDTH, y: index * ROW_HEIGHT + ROW_HEIGHT / 2 });

          commit.parent_hashes.forEach((parent, parentIndex) => {
            if (rows.some(r => r.commit.hash === parent)) return;
            if (active.indexOf(parent) >= 0) return; // 已有泳道在等这个父提交
            let parentLane = parentIndex === 0 ? lane : findFreeLane();
            if (active[parentLane] !== null) parentLane = findFreeLane();
            active[parentLane] = parent;
          });

          while (active.length && active[active.length - 1] === null) active.pop();
        });

        const rowByHash = new Map(rows.map(r => [r.commit.hash, r]));
        positionsRef.current = Object.fromEntries(rows.map(r => [r.commit.hash, { x: r.x, y: r.y }]));

        // 先画连线，节点压在上面
        rows.forEach(row => {
          row.commit.parent_hashes.forEach(parentHash => {
            const parent = rowByHash.get(parentHash);
            if (!parent) return;
            ctx.strokeStyle = PALETTE[parent.lane % PALETTE.length];
            ctx.lineWidth = 1.8;
            ctx.beginPath();
            ctx.moveTo(row.x, row.y);
            if (parent.lane === row.lane) {
              ctx.lineTo(parent.x, parent.y);
            } else {
              // 换道的连线走 S 曲线，接近 git 客户端的观感
              const midY = (row.y + parent.y) / 2;
              ctx.bezierCurveTo(row.x, midY, parent.x, midY, parent.x, parent.y);
            }
            ctx.stroke();
          });
        });

        rows.forEach(row => {
          const color = PALETTE[row.lane % PALETTE.length];
          const isMerge = row.commit.parent_hashes.length > 1;
          if (isMerge) {
            // 合并提交画成空心环 + 圆心，和普通提交区分开
            ctx.beginPath();
            ctx.arc(row.x, row.y, 5.2, 0, Math.PI * 2);
            ctx.fillStyle = bg;
            ctx.fill();
            ctx.strokeStyle = color;
            ctx.lineWidth = 2.2;
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(row.x, row.y, 2, 0, Math.PI * 2);
            ctx.fillStyle = color;
            ctx.fill();
          } else {
            ctx.beginPath();
            ctx.arc(row.x, row.y, 4.2, 0, Math.PI * 2);
            ctx.fillStyle = color;
            ctx.fill();
            ctx.strokeStyle = bg;
            ctx.lineWidth = 1.5;
            ctx.stroke();
          }
        });

        // 文字放最后画，避免被连线压住
        const laneCount = rows.reduce((max, r) => Math.max(max, r.lane), 0) + 1;
        const textX = Math.max(LANE_LEFT + laneCount * LANE_WIDTH + 10, 110);
        rows.forEach(row => {
          ctx.font = '11px ui-monospace, SFMono-Regular, Menlo, monospace';
          ctx.fillStyle = faintColor;
          const hash = row.commit.hash.substring(0, 8);
          ctx.fillText(hash, textX, row.y + 4);

          ctx.font = '12.5px -apple-system, BlinkMacSystemFont, sans-serif';
          ctx.fillStyle = textColor;
          const messageX = textX + ctx.measureText('00000000').width + 26;
          let message = row.commit.message.split('\n')[0] || '(无提交信息)';
          const maxWidth = width - messageX - 14;
          if (maxWidth > 40 && ctx.measureText(message).width > maxWidth) {
            while (message.length > 1 && ctx.measureText(`${message}…`).width > maxWidth) {
              message = message.slice(0, -1);
            }
            message += '…';
          }
          ctx.fillText(message, messageX, row.y + 4);
        });

        if (commits.length === 0) {
          ctx.font = '13px -apple-system, BlinkMacSystemFont, sans-serif';
          ctx.fillStyle = faintColor;
          ctx.fillText('没有提交可显示', 16, 30);
        }
      } catch (e: any) {
        setError(String(e));
      }
    };

    loadGraph();
  }, [repoPath]);

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <h3>提交图</h3>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      <div ref={containerRef} style={{ maxHeight: 500, overflowY: 'auto' }}>
        <canvas
          ref={canvasRef}
          onClick={(e) => {
            const canvas = canvasRef.current;
            if (!canvas) return;
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            let best: { hash: string; dist: number } | null = null;
            for (const [hash, pos] of Object.entries(positionsRef.current)) {
              const d = Math.hypot(pos.x - x, pos.y - y);
              if (d <= 14 && (!best || d < best.dist)) best = { hash, dist: d };
            }
            if (best) onSelectRef.current(best.hash);
          }}
          style={{ cursor: 'pointer', display: 'block' }}
        />
      </div>
    </motion.div>
  );
}

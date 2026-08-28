import { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface GraphCommit {
  hash: string;
  author: string;
  time: string;
  message: string;
  parent_hashes: string[];
}

export default function GraphView({ repoPath, onSelectCommit }: { repoPath: string; onSelectCommit: (hash: string) => void }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const positionsRef = useRef<Record<string, { x: number; y: number }>>({});
  const commitsRef = useRef<GraphCommit[]>([]);

  useEffect(() => {
    const loadGraph = async () => {
      const commits = await invoke<GraphCommit[]>('get_graph_commits', { path: repoPath });
      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      
      canvas.width = containerRef.current?.clientWidth || 800;
      canvas.height = commits.length * 40;
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      
      const positions: Record<string, { x: number; y: number }> = {};
      positionsRef.current = positions;
      commitsRef.current = commits;

      commits.forEach((c, i) => {
        const y = i * 40 + 20;
        const x = 60 + (c.parent_hashes.length > 1 ? 20 : 0);
        positions[c.hash] = { x, y };
      });
      
      commits.forEach((c, i) => {
        const y = i * 40 + 20;
        const x = positions[c.hash]?.x || 60;
        
        c.parent_hashes.forEach(pHash => {
          const pPos = positions[pHash];
          if (pPos) {
            ctx.strokeStyle = '#5B9BD5';
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(x, y);
            ctx.lineTo(pPos.x, pPos.y);
            ctx.stroke();
          }
        });
        
        ctx.fillStyle = '#5B9BD5';
        ctx.beginPath();
        ctx.arc(x, y, 6, 0, Math.PI * 2);
        ctx.fill();
        
        ctx.fillStyle = '#c8d6e5';
        ctx.font = '12px -apple-system, BlinkMacSystemFont, sans-serif';
        ctx.fillText(`${c.hash.substring(0, 8)} - ${c.message.substring(0, 40)}`, x + 14, y + 4);
      });
    };
    
    loadGraph();
  }, [repoPath]);

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>提交图</h3>
      <div ref={containerRef} style={{ maxHeight: 500, overflowY: 'auto' }}>
        <canvas
          ref={canvasRef}
          onClick={(e) => {
            // 点击最近的提交圆点（半径 6，放宽到 14px 命中范围）
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
            if (best) onSelectCommit(best.hash);
          }}
          style={{ cursor: 'pointer' }}
        />
      </div>
    </motion.div>
  );
}
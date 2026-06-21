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

export default function GraphView({ repoPath, onSelectCommit: _onSelectCommit }: { repoPath: string; onSelectCommit: (hash: string) => void }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

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
        ctx.font = '12px Inter, sans-serif';
        ctx.fillText(`${c.hash.substring(0, 8)} - ${c.message.substring(0, 40)}`, x + 14, y + 4);
      });
    };
    
    loadGraph();
  }, [repoPath]);

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>提交图</h3>
      <div ref={containerRef} style={{ maxHeight: 500, overflowY: 'auto' }}>
        <canvas ref={canvasRef} />
      </div>
    </motion.div>
  );
}
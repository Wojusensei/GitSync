import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';

interface Command {
  id: string;
  label: string;
  action: () => void;
}

export default function CommandPalette({ commands }: { commands: Command[] }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');

  const filtered = commands.filter(c => c.label.toLowerCase().includes(query.toLowerCase()));

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setOpen(true);
      }
      if (e.key === 'Escape') setOpen(false);
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, []);

  if (!open) return null;

  return (
    <div style={{ position: 'fixed', top: 0, left: 0, width: '100%', height: '100%', zIndex: 9999, background: 'rgba(0,0,0,0.6)', display: 'flex', justifyContent: 'center', paddingTop: 100 }}>
      <motion.div initial={{ scale: 0.9 }} animate={{ scale: 1 }} className="analysis-panel" style={{ width: 500, maxHeight: 400, overflowY: 'auto' }}>
        <input className="path-input" value={query} onChange={e => setQuery(e.target.value)} placeholder="输入命令..." autoFocus style={{ marginBottom: 12 }} />
        {filtered.map(c => (
          <div key={c.id} className="analysis-item" onClick={() => { c.action(); setOpen(false); }} style={{ cursor: 'pointer' }}>{c.label}</div>
        ))}
      </motion.div>
    </div>
  );
}
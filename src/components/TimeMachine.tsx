import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface TimeMachineFile {
  path: string;
  size: number;
  is_directory: boolean;
}

interface TimeMachineSnapshot {
  commit_hash: string;
  author: string;
  time: string;
  message: string;
  files: TimeMachineFile[];
}

export default function TimeMachine({ repoPath }: { repoPath: string }) {
  const [snapshot, setSnapshot] = useState<TimeMachineSnapshot | null>(null);
  const [fileContent, setFileContent] = useState('');
  const [selectedFile, setSelectedFile] = useState('');
  const [currentTime, setCurrentTime] = useState(Math.floor(Date.now() / 1000));
  const [isPlaying, setIsPlaying] = useState(false);
  const [playSpeed, setPlaySpeed] = useState(1);
  const [minTime, setMinTime] = useState(0);
  const [maxTime, setMaxTime] = useState(Math.floor(Date.now() / 1000));
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    const loadTimeRange = async () => {
      try {
        const commits: any[] = await invoke('get_commits', { path: repoPath });
        if (commits.length > 0) {
          const times = commits.map((c: any) => new Date(c.time).getTime() / 1000);
          setMinTime(Math.min(...times));
          setMaxTime(Math.max(...times));
          setCurrentTime(Math.max(...times));
        }
      } catch (e) { console.error(e); }
    };
    loadTimeRange();
  }, [repoPath]);

  const loadSnapshot = async (ts: number) => {
    try {
      const res: TimeMachineSnapshot = await invoke('get_time_machine_snapshot', { path: repoPath, timestamp: ts });
      setSnapshot(res);
    } catch (e) { console.error(e); }
  };

  useEffect(() => {
    if (repoPath) loadSnapshot(currentTime);
  }, [repoPath]);

  const handleSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const ts = parseInt(e.target.value);
    setCurrentTime(ts);
    loadSnapshot(ts);
  };

  const startPlay = () => {
    if (isPlaying) return;
    setIsPlaying(true);
    intervalRef.current = setInterval(() => {
      setCurrentTime(prev => {
        const next = prev + playSpeed * 10;
        if (next >= maxTime) {
          clearInterval(intervalRef.current!);
          setIsPlaying(false);
          return maxTime;
        }
        loadSnapshot(next);
        return next;
      });
    }, 200);
  };

  const stopPlay = () => {
    if (intervalRef.current) clearInterval(intervalRef.current);
    setIsPlaying(false);
  };

  const loadFileContent = async (filePath: string) => {
    if (!snapshot) return;
    setSelectedFile(filePath);
    try {
      const content: string = await invoke('get_file_content_at_commit', {
        path: repoPath,
        commitHash: snapshot.commit_hash,
        filePath,
      });
      setFileContent(content);
    } catch (e) { setFileContent('无法加载文件内容'); }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>时间机器</h3>

      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 12 }}>
        <button className="btn" onClick={stopPlay} disabled={!isPlaying} style={{ padding: '4px 12px' }}>⏸</button>
        <button className="btn btn-blue" onClick={startPlay} disabled={isPlaying} style={{ padding: '4px 12px' }}>▶</button>
        <select value={playSpeed} onChange={e => setPlaySpeed(Number(e.target.value))} style={{ background: '#3c3c3c', color: '#d4d4d4', border: '1px solid #555', borderRadius: 4, padding: '4px 8px' }}>
          <option value={1}>1x</option>
          <option value={5}>5x</option>
          <option value={20}>20x</option>
        </select>
        <span style={{ fontSize: 13, color: '#8899aa', marginLeft: 'auto' }}>
          {snapshot?.time || '加载中...'}
        </span>
      </div>

      <input
        type="range"
        min={minTime}
        max={maxTime}
        value={currentTime}
        onChange={handleSliderChange}
        style={{ width: '100%', marginTop: 12 }}
      />

      {snapshot && (
        <div style={{ marginTop: 12 }}>
          <div className="analysis-item">
            <span className="hash">{snapshot.commit_hash.substring(0, 8)}</span>
            <span className="author">{snapshot.author}</span>
            <span className="message" style={{ marginLeft: 8 }}>{snapshot.message}</span>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginTop: 12 }}>
            <div>
              <div className="section-title">文件树</div>
              <div style={{ maxHeight: 300, overflowY: 'auto' }}>
                {snapshot.files.map(f => (
                  <div
                    key={f.path}
                    className="analysis-item"
                    onClick={() => !f.is_directory && loadFileContent(f.path)}
                    style={{ cursor: f.is_directory ? 'default' : 'pointer', display: 'flex', justifyContent: 'space-between' }}
                  >
                    <span>{f.is_directory ? '📁' : '📄'} {f.path}</span>
                    {!f.is_directory && <span style={{ fontSize: 11, color: '#8899aa' }}>{(f.size / 1024).toFixed(1)} KB</span>}
                  </div>
                ))}
              </div>
            </div>
            <div>
              <div className="section-title">代码预览</div>
              <pre style={{
                background: 'rgba(255,255,255,0.03)',
                border: '1px solid rgba(255,255,255,0.06)',
                borderRadius: 8,
                padding: 12,
                maxHeight: 300,
                overflow: 'auto',
                fontSize: 12,
                whiteSpace: 'pre-wrap',
                color: '#c8d6e5',
              }}>
                {fileContent || (selectedFile ? '加载中...' : '点击文件查看代码')}
              </pre>
            </div>
          </div>
        </div>
      )}
    </motion.div>
  );
}
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { VscSymbolColor, VscLightbulb, VscFileMedia, VscColorMode, VscLayout, VscCheck } from 'react-icons/vsc';

interface UIManagerProps {
  bgOpacity: number;
  setBgOpacity: (v: number) => void;
  setBgBase64: (v: string) => void;
  defaultBgBase64: string;
  panelMode: 'stack' | 'replace';
  setPanelMode: (v: 'stack' | 'replace') => void;
  theme: string;
  setTheme: (v: string) => void;
  torchSize: number;
  setTorchSize: (v: number) => void;
}

export default function UIManager({
  bgOpacity, setBgOpacity,
  setBgBase64,
  defaultBgBase64,
  panelMode, setPanelMode,
  theme, setTheme,
  torchSize, setTorchSize,
}: UIManagerProps) {
  const [customBgError, setCustomBgError] = useState('');
  const [bgPreview, setBgPreview] = useState<string | null>(null);

  const themes = [
    { id: 'dark', label: '深色', bg: '#0a0a14' },
    { id: 'light', label: '浅色', bg: '#f0f0f0' },
    { id: 'soft', label: '柔和', bg: '#e6e2d9' },
  ];

  const handlePickBackground = async () => {
    setCustomBgError('');
    setBgPreview(null);
    try {
      const b64 = await invoke<string>('pick_background_image');
      if (b64) {
        setBgBase64(b64);
        localStorage.setItem('bg_base64', b64);
        setBgPreview(b64);
      }
    } catch (e: any) {
      setCustomBgError(String(e));
    }
  };

  const handleResetBackground = () => {
    setBgBase64(defaultBgBase64);
    localStorage.setItem('bg_base64', defaultBgBase64);
    setBgPreview(null);
    setCustomBgError('');
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3 style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <VscLayout size={18} />
        UI 管理
      </h3>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscSymbolColor size={14} style={{ color: '#8899aa' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>背景遮罩强度</span>
          <span style={{ marginLeft: 'auto', fontSize: 12, color: '#8899aa' }}>{Math.round(bgOpacity * 100)}%</span>
        </div>
        <input
          type="range"
          min="0"
          max="100"
          value={Math.round(bgOpacity * 100)}
          onChange={e => {
            const val = Number(e.target.value) / 100;
            setBgOpacity(val);
            localStorage.setItem('bg_opacity', String(val));
          }}
          style={{ width: '100%', accentColor: '#5B9BD5' }}
        />
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscLightbulb size={14} style={{ color: '#8899aa' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>手电筒范围</span>
          <span style={{ marginLeft: 'auto', fontSize: 12, color: '#8899aa' }}>{torchSize}px</span>
        </div>
        <input
          type="range"
          min="30"
          max="300"
          value={torchSize}
          onChange={e => {
            const val = Number(e.target.value);
            setTorchSize(val);
            localStorage.setItem('torch_size', String(val));
          }}
          style={{ width: '100%', accentColor: '#5B9BD5' }}
        />
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscFileMedia size={14} style={{ color: '#8899aa' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>自定义背景</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <div style={{ display: 'flex', gap: 8 }}>
            <button
              className="btn btn-blue"
              onClick={handlePickBackground}
              style={{ padding: '6px 14px', fontSize: 12 }}
            >
              选择图片
            </button>
            <button
              className="btn"
              onClick={handleResetBackground}
              style={{
                background: 'rgba(255,255,255,0.05)',
                border: '1px solid rgba(255,255,255,0.1)',
                borderRadius: 6,
                color: '#c8d6e5',
                cursor: 'pointer',
                padding: '6px 14px',
                fontSize: 12
              }}
            >
              重置默认
            </button>
          </div>
          <div
            style={{
              width: 48,
              height: 48,
              borderRadius: 8,
              overflow: 'hidden',
              border: '1px solid rgba(255,255,255,0.1)',
              flexShrink: 0,
              background: '#1a1a2e',
            }}
          >
            <img
              src={`data:image/jpeg;base64,${bgPreview || defaultBgBase64}`}
              alt="背景预览"
              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
            />
          </div>
        </div>
        {customBgError && <div style={{ color: '#ff6b6b', fontSize: 12, marginTop: 6 }}>{customBgError}</div>}
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscColorMode size={14} style={{ color: '#8899aa' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>主题</span>
        </div>
        <div style={{ display: 'flex', gap: 10 }}>
          {themes.map(t => (
            <button
              key={t.id}
              onClick={() => {
                setTheme(t.id);
                localStorage.setItem('theme', t.id);
                document.documentElement.setAttribute('data-theme', t.id);
              }}
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                gap: 4,
                background: 'transparent',
                border: 'none',
                cursor: 'pointer',
                padding: 0,
              }}
            >
              <div
                style={{
                  width: 36,
                  height: 36,
                  borderRadius: '50%',
                  background: t.bg,
                  border: theme === t.id ? '2px solid #5B9BD5' : '2px solid rgba(255,255,255,0.15)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  transition: 'border 0.2s',
                }}
              >
                {theme === t.id && <VscCheck size={16} style={{ color: t.id === 'dark' ? '#5B9BD5' : '#333' }} />}
              </div>
              <span style={{ fontSize: 10, color: '#8899aa' }}>{t.label}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscLayout size={14} style={{ color: '#8899aa' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>面板模式</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <span style={{ fontSize: 13, color: '#c8d6e5' }}>
            {panelMode === 'stack' ? '叠加模式' : '替换模式'}
          </span>
          <button
            onClick={() => {
              const next = panelMode === 'stack' ? 'replace' : 'stack';
              setPanelMode(next);
              localStorage.setItem('panel_mode', next);
            }}
            style={{
              position: 'relative',
              width: 44,
              height: 24,
              borderRadius: 12,
              background: panelMode === 'stack' ? '#5B9BD5' : 'rgba(255,255,255,0.15)',
              border: 'none',
              cursor: 'pointer',
              transition: 'background 0.25s',
              flexShrink: 0,
            }}
          >
            <div
              style={{
                position: 'absolute',
                top: 2,
                left: panelMode === 'stack' ? 22 : 2,
                width: 20,
                height: 20,
                borderRadius: '50%',
                background: '#fff',
                transition: 'left 0.25s',
                boxShadow: '0 1px 4px rgba(0,0,0,0.3)',
              }}
            />
          </button>
        </div>
        <div style={{ fontSize: 11, color: '#8899aa', marginTop: 6, lineHeight: 1.5 }}>
          {panelMode === 'replace'
            ? '替换模式：点击新功能时自动关闭之前的面板，只显示当前选中项'
            : '叠加模式：多个面板可同时展开，新面板出现时自动滚动到视图'}
        </div>
      </div>

      <style>{`
        :root[data-theme='light'] { --bg: #f0f0f0; --text: #222; }
        :root[data-theme='soft'] { --bg: #e6e2d9; --text: #333; }
        :root[data-theme='dark'] { --bg: #0a0a14; --text: #c8d6e5; }
        body { background: var(--bg); color: var(--text); }
      `}</style>
    </motion.div>
  );
}
import { useState } from 'react';
import { motion } from 'framer-motion';
import { VscSymbolColor, VscLightbulb, VscFileMedia, VscColorMode, VscLayout, VscCheck } from 'react-icons/vsc';
import { pickBackgroundImage } from '../utils/backgroundImage';
import ImageCropper from './ImageCropper';

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
  glassBlur: number;
  setGlassBlur: (v: number) => void;
  bgStyle: 'preset' | 'md' | 'custom';
  setBgStyle: (v: 'preset' | 'md' | 'custom') => void;
  bgBase64: string;   // 自定义背景缩略图
}

export default function UIManager({
  bgOpacity, setBgOpacity,
  setBgBase64,
  defaultBgBase64,
  panelMode, setPanelMode,
  theme, setTheme,
  torchSize, setTorchSize,
  glassBlur, setGlassBlur,
  bgStyle, setBgStyle,
  bgBase64,
}: UIManagerProps) {
  const [customBgError, setCustomBgError] = useState('');
  const [bgPreview, setBgPreview] = useState<string | null>(null);
  const [cropSrc, setCropSrc] = useState<string | null>(null);
  const themes = [
    { id: 'dark', label: '深色', bg: '#0a0a14' },
    { id: 'light', label: '浅色', bg: '#f0f0f0' },
    { id: 'soft', label: '柔和', bg: '#e6e2d9' },
  ];

  const bgStyleOptions = [
    { id: 'preset' as const, label: '官方预设', desc: '蔚蓝档案风格' },
    { id: 'md' as const, label: 'MD 纯色', desc: 'Material Design 柔和渐变' },
    { id: 'custom' as const, label: '自定义图片', desc: '上传你的背景图' },
  ];

  const handlePickBackground = async () => {
    setCustomBgError('');
    setBgPreview(null);
    try {
      const b64 = await pickBackgroundImage();
      // 先框选，确认后再落盘
      setCropSrc(b64);
    } catch (e: any) {
      // 用户在文件选择器里点取消不算错误，静默返回
      const msg = String(e);
      if (!msg.includes('取消')) {
        setCustomBgError(msg);
      }
    }
  };

  const handleResetBackground = () => {
    setBgBase64(defaultBgBase64);
    localStorage.setItem('bg_base64', defaultBgBase64);
    setBgPreview(null);
    setCustomBgError('');
    setBgStyle('preset');
    localStorage.setItem('bgStyle', 'preset');
  };

  const handleBgStyleChange = async (style: 'preset' | 'md' | 'custom') => {
    if (style === 'custom') {
      await handlePickBackground();
      return;
    }
    if (style === 'preset') {
      // 选回预设要同时把自定义图片换回默认，否则背景还停在自定义图
      setBgBase64(defaultBgBase64);
      localStorage.setItem('bg_base64', defaultBgBase64);
      setBgPreview(null);
    }
    setBgStyle(style);
    localStorage.setItem('bgStyle', style);
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <h3 style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <VscLayout size={18} />
        UI 管理
      </h3>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscColorMode size={14} style={{ color: 'var(--text-dim)' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>背景风格</span>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 10 }}>
          {bgStyleOptions.map((option) => (
            <button
              key={option.id}
              onClick={() => handleBgStyleChange(option.id)}
              style={{
                background: bgStyle === option.id ? 'var(--accent-soft)' : 'var(--panel)',
                border: bgStyle === option.id ? '1px solid var(--accent-border)' : '1px solid var(--border)',
                borderRadius: 12,
                padding: '10px 8px 8px',
                cursor: 'pointer',
                transition: 'all 0.25s ease',
                textAlign: 'center',
                color: 'var(--text)',
              }}
              onMouseEnter={(e) => {
                if (bgStyle !== option.id) {
                  e.currentTarget.style.background = 'var(--panel-strong)';
                  e.currentTarget.style.borderColor = 'var(--border-strong)';
                }
              }}
              onMouseLeave={(e) => {
                if (bgStyle !== option.id) {
                  e.currentTarget.style.background = 'var(--panel)';
                  e.currentTarget.style.borderColor = 'var(--border)';
                }
              }}
            >
              <div
                style={{
                  width: '100%',
                  height: 56,
                  borderRadius: 8,
                  overflow: 'hidden',
                  marginBottom: 6,
                }}
              >
                {option.id === 'preset' && (
                  <img
                    src={`data:image/jpeg;base64,${defaultBgBase64}`}
                    alt="官方预设"
                    style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                  />
                )}
                {option.id === 'md' && (
                  <div style={{ width: '100%', height: '100%', background: 'linear-gradient(145deg, var(--grad-a), var(--grad-b), var(--grad-c))' }} />
                )}
                {option.id === 'custom' && (
                  bgBase64 ? (
                    <img
                      src={`data:image/jpeg;base64,${bgBase64}`}
                      alt="自定义"
                      style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                    />
                  ) : (
                    <div style={{
                      width: '100%',
                      height: '100%',
                      background: 'var(--panel)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: 24,
                      color: 'var(--text-faint)',
                    }}>
                      +
                    </div>
                  )
                )}
              </div>
              <div style={{ fontSize: 12, fontWeight: 600 }}>{option.label}</div>
              <div style={{ fontSize: 10, color: 'var(--text-dim)', marginTop: 1 }}>{option.desc}</div>
            </button>
          ))}
        </div>
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscSymbolColor size={14} style={{ color: 'var(--text-dim)' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>背景遮罩强度</span>
          <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text-dim)' }}>{Math.round(bgOpacity * 100)}%</span>
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
          style={{ width: '100%', accentColor: 'var(--accent)' }}
        />
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscLightbulb size={14} style={{ color: 'var(--text-dim)' }} />
            <span className="section-title" style={{ marginBottom: 0 }}>手电筒范围</span>
          <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text-dim)' }}>{torchSize}px</span>
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
          style={{ width: '100%', accentColor: 'var(--accent)' }}
        />

      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscSymbolColor size={14} style={{ color: 'var(--text-dim)' }} />
          <span className="section-title" style={{ marginBottom: 0 }}>玻璃通透度</span>
          <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text-dim)' }}>{glassBlur}px</span>
        </div>
        <input
          type="range"
          min="0"
          max="20"
          value={glassBlur}
          onChange={e => {
            const val = Number(e.target.value);
            setGlassBlur(val);
            localStorage.setItem('glass_blur', String(val));
          }}
          style={{ width: '100%', accentColor: 'var(--accent)' }}
        />
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: 'var(--text-dim)', marginTop: 4 }}>
          <span>通透</span>
          <span>磨砂</span>
        </div>
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscFileMedia size={14} style={{ color: 'var(--text-dim)' }} />
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
                background: 'var(--panel)',
                border: '1px solid var(--border)',
                borderRadius: 6,
                color: 'var(--text)',
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
              border: '1px solid var(--border-strong)',
              flexShrink: 0,
              background: 'var(--grad-a)',
            }}
          >
            <img
              src={`data:image/jpeg;base64,${bgPreview || defaultBgBase64}`}
              alt="背景预览"
              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
            />
          </div>
        </div>
        {customBgError && (
          <div style={{ marginTop: 10, color: 'var(--danger)', fontSize: 12 }}>
            {customBgError}
          </div>
        )}
      </div>

      <div className="analysis-section" style={{ marginTop: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <VscColorMode size={14} style={{ color: 'var(--text-dim)' }} />
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
                  border: theme === t.id ? '2px solid var(--accent)' : '2px solid var(--border-strong)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    transition: 'border 0.2s',
                  }}
                >
                  {theme === t.id && <VscCheck size={16} style={{ color: t.id === 'dark' ? 'var(--accent)' : 'var(--text)' }} />}
                </div>
                <span style={{ fontSize: 10, color: 'var(--text-dim)' }}>{t.label}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="analysis-section" style={{ marginTop: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
            <VscLayout size={14} style={{ color: 'var(--text-dim)' }} />
            <span className="section-title" style={{ marginBottom: 0 }}>面板模式</span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
            <span style={{ fontSize: 13, color: 'var(--text)' }}>
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
                background: panelMode === 'stack' ? 'var(--accent)' : 'var(--border-strong)',
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
          <div style={{ fontSize: 11, color: 'var(--text-dim)', marginTop: 6, lineHeight: 1.5 }}>
            {panelMode === 'replace'
              ? '替换模式：点击新功能时自动关闭之前的面板，只显示当前选中项'
              : '叠加模式：多个面板可同时展开，新面板出现时自动滚动到视图'}
          </div>
        </div>

      {cropSrc && (
        <ImageCropper
          src={cropSrc}
          onCancel={() => setCropSrc(null)}
          onConfirm={(b64) => {
            setBgBase64(b64);
            localStorage.setItem('bg_base64', b64);
            setBgPreview(b64);
            setBgStyle('custom');
            localStorage.setItem('bgStyle', 'custom');
            setCropSrc(null);
          }}
        />
      )}
    </motion.div>
  );
}
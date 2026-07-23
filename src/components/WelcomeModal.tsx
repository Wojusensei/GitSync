import { motion, AnimatePresence } from 'framer-motion';
import { VscClose } from 'react-icons/vsc';

interface WelcomeModalProps {
  isOpen: boolean;
  onSelect: (style: 'preset' | 'md' | 'custom') => void;
  onClose: () => void;
}

export default function WelcomeModal({ isOpen, onSelect, onClose }: WelcomeModalProps) {
  if (!isOpen) return null;

  return (
    <AnimatePresence>
      <motion.div
        className="glass-overlay"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.3 }}
      >
        <motion.div
          className="glass-modal"
          initial={{ scale: 0.92, y: 20, opacity: 0 }}
          animate={{ scale: 1, y: 0, opacity: 1 }}
          exit={{ scale: 0.92, y: 20, opacity: 0 }}
          transition={{ duration: 0.35, ease: 'easeOut' }}
        >
          <div className="glass-modal-header">
            <h3 style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <span style={{ fontSize: 22 }}>🎨</span>
              选择你的背景风格
            </h3>
            <button
              className="glass-close-btn"
              onClick={onClose}
              aria-label="关闭"
            >
              <VscClose size={20} />
            </button>
          </div>

          <div className="glass-modal-body" style={{ paddingTop: 8 }}>
            <p style={{ fontSize: 14, color: 'rgba(255,255,255,0.6)', marginBottom: 4, lineHeight: 1.6 }}>
              感谢您使用我们的产品desuwa！选择你喜欢的界面背景风格，后续还可在 <strong>UI 管理</strong> 面板中随时切换哦awa
            </p>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 14, marginTop: 6 }}>
              {/* <^预设蓝色恶魔！&> */}
              <button
                className="welcome-option"
                onClick={() => onSelect('preset')}
                style={{
                  background: 'rgba(255,255,255,0.04)',
                  border: '1px solid rgba(255,255,255,0.08)',
                  borderRadius: 16,
                  padding: '16px 12px 14px',
                  cursor: 'pointer',
                  transition: 'all 0.25s ease',
                  textAlign: 'center',
                  color: '#c8d6e5',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.18)';
                  e.currentTarget.style.transform = 'translateY(-2px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'rgba(255,255,255,0.04)';
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                <div
                  style={{
                    width: '100%',
                    height: 80,
                    borderRadius: 10,
                    background: 'linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%)',
                    marginBottom: 10,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: 32,
                    opacity: 0.7,
                  }}
                >
                  🌊
                </div>
                <div style={{ fontWeight: 600, fontSize: 14 }}>官方预设</div>
                <div style={{ fontSize: 11, color: 'rgba(255,255,255,0.4)', marginTop: 2 }}>蔚蓝档案风格</div>
              </button>

              {/* <^MD2&> */}
              <button
                className="welcome-option"
                onClick={() => onSelect('md')}
                style={{
                  background: 'rgba(255,255,255,0.04)',
                  border: '1px solid rgba(255,255,255,0.08)',
                  borderRadius: 16,
                  padding: '16px 12px 14px',
                  cursor: 'pointer',
                  transition: 'all 0.25s ease',
                  textAlign: 'center',
                  color: '#c8d6e5',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.18)';
                  e.currentTarget.style.transform = 'translateY(-2px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'rgba(255,255,255,0.04)';
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                <div
                  style={{
                    width: '100%',
                    height: 80,
                    borderRadius: 10,
                    background: 'linear-gradient(145deg, #1a1a2e, #16213e, #0f3460)',
                    marginBottom: 10,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: 32,
                    opacity: 0.7,
                  }}
                >
                  🎨
                </div>
                <div style={{ fontWeight: 600, fontSize: 14 }}>MD 纯色</div>
                <div style={{ fontSize: 11, color: 'rgba(255,255,255,0.4)', marginTop: 2 }}>Material Design 柔和渐变</div>
              </button>

              {/* <^自定义&> */}
              <button
                className="welcome-option"
                onClick={() => onSelect('custom')}
                style={{
                  background: 'rgba(255,255,255,0.04)',
                  border: '1px solid rgba(255,255,255,0.08)',
                  borderRadius: 16,
                  padding: '16px 12px 14px',
                  cursor: 'pointer',
                  transition: 'all 0.25s ease',
                  textAlign: 'center',
                  color: '#c8d6e5',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.18)';
                  e.currentTarget.style.transform = 'translateY(-2px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'rgba(255,255,255,0.04)';
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                <div
                  style={{
                    width: '100%',
                    height: 80,
                    borderRadius: 10,
                    background: 'rgba(255,255,255,0.05)',
                    marginBottom: 10,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: 32,
                    opacity: 0.7,
                  }}
                >
                  🖼️
                </div>
                <div style={{ fontWeight: 600, fontSize: 14 }}>自定义图片</div>
                <div style={{ fontSize: 11, color: 'rgba(255,255,255,0.4)', marginTop: 2 }}>上传你的背景图</div>
              </button>
            </div>
          </div>

          <div className="glass-modal-footer">
            <button
              className="btn"
              onClick={onClose}
              style={{
                background: 'rgba(255,255,255,0.05)',
                border: '1px solid rgba(255,255,255,0.1)',
                color: 'rgba(255,255,255,0.6)',
                padding: '8px 20px',
                borderRadius: 10,
                fontSize: 13,
                cursor: 'pointer',
                transition: 'all 0.2s ease',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = 'rgba(255,255,255,0.1)';
                e.currentTarget.style.color = '#ffffff';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'rgba(255,255,255,0.05)';
                e.currentTarget.style.color = 'rgba(255,255,255,0.6)';
              }}
            >
              稍后选择，先尝尝咸淡？
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
import { AnimatePresence, motion } from 'framer-motion';
import { ReactNode } from 'react';

interface GlassModalProps {
  isOpen: boolean;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  onClose: () => void;
}

export default function GlassModal({ isOpen, title, children, footer, onClose }: GlassModalProps) {
  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="glass-overlay"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.25 }}
          onClick={onClose}
        >
          <motion.div
            className="glass-modal"
            initial={{ opacity: 0, y: 24, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 24, scale: 0.96 }}
            transition={{ type: 'spring', stiffness: 260, damping: 24 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="glass-modal-header">
              <h3>{title}</h3>
              <button className="glass-close-btn" onClick={onClose} aria-label="关闭">×</button>
            </div>
            <div className="glass-modal-body">{children}</div>
            {footer && <div className="glass-modal-footer">{footer}</div>}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

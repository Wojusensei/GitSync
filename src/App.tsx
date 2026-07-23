import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion, AnimatePresence, useMotionValue, useSpring } from 'framer-motion';
import { SiGit } from 'react-icons/si';
import { VscRepoForked, VscGitCommit, VscSourceControl, VscEmptyWindow, VscDiffAdded, VscDiffRemoved, VscSearch, VscFileCode, VscHistory, VscChevronRight, VscFolderOpened } from 'react-icons/vsc';
import './App.css';
import { addSafeDirectory } from './services/tauriService';
import FeedbackOverlay from './components/FeedbackOverlay';
import { DEFAULT_BG_BASE64 } from './assets/defaultBg';
import { parseAndRenderDiff } from './utils/diffUtils';
import WelcomeModal from './components/WelcomeModal';  // <^welcome弹窗组件!thanks to @GuGulsNotAPigeon &>

import SemanticSearch from './components/SemanticSearch';
import DiffViewer from './components/DiffViewer';
import ChangelogGenerator from './components/ChangelogGenerator';
import EnhancedRebase from './components/EnhancedRebase';
import CommandPalette from './components/CommandPalette';
import GraphView from './components/GraphView';
import FileTree from './components/FileTree';
import FileViewer from './components/FileViewer';
import CommitFilter from './components/CommitFilter';
import TagManager from './components/TagManager';
import RemoteManager from './components/RemoteManager';
import MultiRepo from './components/MultiRepo';
import SyntaxHighlight from './components/SyntaxHighlight';
import SideBySideDiff from './components/SideBySideDiff';
import HookManager from './components/HookManager';
import ConflictResolver from './components/ConflictResolver';
import ExportHTML from './components/ExportHTML';
import ScriptRunner from './components/ScriptRunner';
import QueryConsole from './components/QueryConsole';
import TimeMachine from './components/TimeMachine';
import UIManager from './components/UIManager';
import SafeDirectoryModal from './components/SafeDirectoryModal';

interface Commit {
  hash: string;
  author: string;
  time: string;
  message: string;
}

interface Branch {
  name: string;
  is_head: boolean;
}

interface FileChange {
  path: string;
  status: string;
  additions: number;
  deletions: number;
  diff: string;
}

interface CommitDetail extends Commit {
  files: FileChange[];
}

interface BlameLine {
  line_number: number;
  commit_hash: string;
  author: string;
  time: string;
  content: string;
}

interface FileTimelineEntry {
  commit_hash: string;
  author: string;
  time: string;
  message: string;
  diff: string;
}

interface HealthReport {
  large_files: string[];
  stale_branches: string[];
  conflicts: string[];
}

interface Contributor {
  author: string;
  commits: number;
  additions: number;
  deletions: number;
}

interface HotFile {
  path: string;
  changes: number;
}

interface StashEntry {
  index: number;
  message: string;
}

interface RebaseCommit {
  hash: string;
  message: string;
  author: string;
  time: string;
}

interface RebaseOperation {
  hash: string;
  action: string;
  new_message?: string;
}

const useRipple = () => {
  const [ripples, setRipples] = useState<{ x: number; y: number; id: number }[]>([]);
  let counter = 0;
  const createRipple = (e: React.MouseEvent) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const id = Date.now() + counter++;
    setRipples(prev => [...prev, { x, y, id }]);
    setTimeout(() => setRipples(prev => prev.filter(r => r.id !== id)), 600);
  };
  return { ripples, createRipple };
};


function App() {
  const [repoPath, setRepoPath] = useState('');
  const [commits, setCommits] = useState<Commit[]>([]);
  const [branches, setBranches] = useState<Branch[]>([]);
  const [activeBranch, setActiveBranch] = useState<string>('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [selectedCommit, setSelectedCommit] = useState<string | null>(null);
  const [commitDetail, setCommitDetail] = useState<CommitDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const { ripples, createRipple } = useRipple();

  // 分页状态
  const [hasMore, setHasMore] = useState(true);
  const PAGE_SIZE = 30;

  const [order, setOrder] = useState<number[]>([]);
  const dragConstraintRef = useRef<HTMLDivElement>(null);
  const mainRef = useRef<HTMLDivElement>(null);

  const pointerX = useMotionValue(window.innerWidth / 2);
  const pointerY = useMotionValue(window.innerHeight / 2);
  const springX = useSpring(pointerX, { stiffness: 200, damping: 30 });
  const springY = useSpring(pointerY, { stiffness: 200, damping: 30 });

  // Safe directory state
  const [safeDirModalPath, setSafeDirModalPath] = useState<string | null>(null);
  const [safeDirFixing, setSafeDirFixing] = useState(false);
  const [safeDirFixingError, setSafeDirFixingError] = useState<string | null>(null);

  // File changes browsing state
  const [currentFileIndex, setCurrentFileIndex] = useState(0);
  const [viewMode, setViewMode] = useState<'single' | 'list'>('single');

  // 分页加载函数
  const loadCommitsPage = useCallback(async (pageNum: number, reset: boolean) => {
    if (!repoPath.trim()) return;
    if (!reset && !hasMore) return;
    setLoading(true);
    setError('');
    try {
      const [branchResult] = await Promise.all([
        invoke<Branch[]>('get_branches', { path: repoPath }),
      ]);
      setBranches(branchResult);
      const headBranch = branchResult.find(b => b.is_head);
      if (headBranch) setActiveBranch(headBranch.name);

      const result = await invoke<[Commit[], number]>('get_commits_paginated', {
        path: repoPath,
        page: pageNum,
        pageSize: PAGE_SIZE
      });
      const [pageData, total] = result;
      if (reset) {
        setCommits(pageData);
      } else {
        setCommits(prev => [...prev, ...pageData]);
      }
      setHasMore((pageNum + 1) * PAGE_SIZE < total);
    } catch (e: any) {
      setError(e);
      if (reset) { setCommits([]); }
    } finally { setLoading(false); }
  }, [repoPath, hasMore]);

  const loadRepo = useCallback(async () => {
    if (!repoPath.trim()) return;
    setHasMore(true);
    setCommits([]);
    setSelectedCommit(null);
    setCommitDetail(null);
    setSelectedTreeFile(null);
    await loadCommitsPage(0, true);
  }, [repoPath, loadCommitsPage]);

  useEffect(() => {
    if (error && error.includes("is not owned by current user")) {
      const match = error.match(/repository path '([^']+)' is not owned by current user/);
      if (match) {
        setSafeDirModalPath(match[1]);
      }
    }
  }, [error]);

  const handleFixSafeDirectory = async () => {
    if (!safeDirModalPath) return;
    setSafeDirFixing(true);
    setSafeDirFixingError(null);
    setToast(null);
    try {
      await addSafeDirectory(safeDirModalPath);
      setSafeDirModalPath(null);
      setError('');
      setToast('安全目录已添加，正在重新加载仓库...');
      await loadRepo();
    } catch (e: any) {
      setSafeDirFixingError(String(e));
      setToast('修复失败，请重试');
    } finally {
      setSafeDirFixing(false);
    }
  };

  const [activeBlame, setActiveBlame] = useState<string | null>(null);
  const [blameData, setBlameData] = useState<BlameLine[]>([]);
  const [activeTimeline, setActiveTimeline] = useState<string | null>(null);
  const [timelineData, setTimelineData] = useState<FileTimelineEntry[]>([]);

  // <^+（preset / md / custom）=> localStorage&>
  const [bgStyle, setBgStyle] = useState<'preset' | 'md' | 'custom'>(() => {
    const saved = localStorage.getItem('bgStyle') as 'preset' | 'md' | 'custom' | null;
    return saved || 'preset';
  });

  // UI 状态（全局生效，不会因为切换面板而重置）
  const [bgOpacity, setBgOpacity] = useState(() => Number(localStorage.getItem('bg_opacity') || '0.9'));
  const [bgBase64, setBgBase64] = useState(() => localStorage.getItem('bg_base64') || DEFAULT_BG_BASE64);
  const [panelMode, setPanelMode] = useState<'stack' | 'replace'>(() => (localStorage.getItem('panel_mode') as 'stack' | 'replace') || 'stack');
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') || 'dark');
  const [torchSize, setTorchSize] = useState(() => Number(localStorage.getItem('torch_size') || '100'));

  // 面板显隐状态
  const [showHealth, setShowHealth] = useState(false);
  const [showContributors, setShowContributors] = useState(false);
  const [showHotFiles, setShowHotFiles] = useState(false);
  const [showStash, setShowStash] = useState(false);
  const [showRebase, setShowRebase] = useState(false);
  const [showGraph, setShowGraph] = useState(false);
  const [showFileTree, setShowFileTree] = useState(false);
  const [selectedTreeFile, setSelectedTreeFile] = useState<string | null>(null);
  const [showCommitFilter, setShowCommitFilter] = useState(false);
  const [showTagManager, setShowTagManager] = useState(false);
  const [showRemoteManager, setShowRemoteManager] = useState(false);
  const [showMultiRepo, setShowMultiRepo] = useState(false);
  const [showSyntaxHighlight, setShowSyntaxHighlight] = useState(false);
  const [showSideBySide, setShowSideBySide] = useState(false);
  const [showHookManager, setShowHookManager] = useState(false);
  const [showConflictResolver, setShowConflictResolver] = useState(false);
  const [showExportHTML, setShowExportHTML] = useState(false);
  const [showSemanticSearch, setShowSemanticSearch] = useState(false);
  const [showDiffViewer, setShowDiffViewer] = useState(false);
  const [showChangelog, setShowChangelog] = useState(false);
  const [showScriptRunner, setShowScriptRunner] = useState(false);
  const [showQueryConsole, setShowQueryConsole] = useState(false);
  const [showTimeMachine, setShowTimeMachine] = useState(false);
  const [showUIManager, setShowUIManager] = useState(false);

  // 侧边栏折叠
  const [expandedSections, setExpandedSections] = useState<Record<string, boolean>>({
    history: false,
    trace: false,
    analysis: false,
    actions: false,
    search_export: false,
    advanced: false
  });
  const toggleSection = (section: string) => {
    setExpandedSections(prev => ({ ...prev, [section]: !prev[section] }));
  };

  // 自动滚动状态
  const [scrollToPanelId, setScrollToPanelId] = useState<string | null>(null);

  // 数据状态
  const [healthReport, setHealthReport] = useState<HealthReport | null>(null);
  const [contributors, setContributors] = useState<Contributor[]>([]);
  const [hotFiles, setHotFiles] = useState<HotFile[]>([]);
  const [stashList, setStashList] = useState<StashEntry[]>([]);
  const [_rebaseOps, setRebaseOps] = useState<RebaseOperation[]>([]);

  // 替换模式：关闭所有面板
  const closeAllPanels = () => {
    setShowHealth(false); setShowContributors(false); setShowHotFiles(false);
    setShowStash(false); setShowRebase(false); setShowGraph(false);
    setShowFileTree(false); setShowCommitFilter(false); setShowTagManager(false);
    setShowRemoteManager(false); setShowMultiRepo(false); setShowSyntaxHighlight(false);
    setShowSideBySide(false); setShowHookManager(false); setShowConflictResolver(false);
    setShowExportHTML(false); setShowSemanticSearch(false); setShowDiffViewer(false);
    setShowChangelog(false); setShowScriptRunner(false); setShowQueryConsole(false);
    setShowTimeMachine(false); setShowUIManager(false);
    setSelectedTreeFile(null);
  };

  const switchBranch = useCallback(async (branchName: string) => {
    setActiveBranch(branchName);
    setHasMore(true);
    setCommits([]);
    setSelectedCommit(null);
    setCommitDetail(null);
    await loadCommitsPage(0, true);
  }, [loadCommitsPage]);

  const openFolder = async () => {
    try {
      const path = await invoke<string>('open_folder_dialog');
      if (path) {
        setRepoPath(path);
        setTimeout(() => loadRepo(), 100);
      }
    } catch (e: any) {
      setError(e);
    }
  };

  useEffect(() => {
    if (repoPath) {
      loadRepo();
    }
  }, []);

  useEffect(() => {
    setOrder(commits.map((_, i) => i));
  }, [commits]);

  const handleDragEnd = (fromIndex: number, info: { offset: { x: number; y: number } }) => {
    const moveY = info.offset.y;
    const newOrder = [...order];
    const toIndex = Math.round(fromIndex + moveY / 60);
    if (toIndex >= 0 && toIndex < order.length && toIndex !== fromIndex) {
      const temp = newOrder[fromIndex];
      newOrder.splice(fromIndex, 1);
      newOrder.splice(toIndex, 0, temp);
      setOrder(newOrder);
    }
  };

  useEffect(() => {
    const handleMouse = (e: MouseEvent) => { pointerX.set(e.clientX); pointerY.set(e.clientY); };
    window.addEventListener('mousemove', handleMouse);
    return () => {
      window.removeEventListener('mousemove', handleMouse);
    };
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  useEffect(() => {
    document.documentElement.style.setProperty('--torch-radius', `${torchSize}px`);
  }, [torchSize]);

  useEffect(() => {
    if (scrollToPanelId) {
      const timer = setTimeout(() => {
        const el = document.getElementById(scrollToPanelId);
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
        setScrollToPanelId(null);
      }, 200);
      return () => clearTimeout(timer);
    }
  }, [scrollToPanelId]);

  const handleCommitClick = async (hash: string) => {
    if (selectedCommit === hash) { setSelectedCommit(null); setCommitDetail(null); return; }
    setSelectedCommit(hash);
    setCurrentFileIndex(0);
    setDetailLoading(true);
    setToast(null);
    try {
      const detail = await invoke<CommitDetail>('get_commit_detail', { path: repoPath, commitHash: hash });
      setCommitDetail(detail);
    } catch (e: any) { setError(String(e)); } finally { setDetailLoading(false); }
  };

  const handleSearch = async () => {
    if (!repoPath.trim()) return;
    if (!searchQuery.trim()) { loadRepo(); return; }
    setLoading(true);
    setToast(null);
    try {
      const result = await invoke<Commit[]>('search_commits', { path: repoPath, query: searchQuery });
      setCommits(result);
      setSelectedCommit(null);
      setCommitDetail(null);
    } catch (e: any) { setError(String(e)); } finally { setLoading(false); }
  };

  const handleBlame = async (filePath: string) => {
    if (activeBlame === filePath) { setActiveBlame(null); setBlameData([]); return; }
    setActiveBlame(filePath);
    setActiveTimeline(null);
    try { const result = await invoke<BlameLine[]>('get_blame', { path: repoPath, filePath }); setBlameData(result); } catch (e: any) { setError(e); }
  };

  const handleTimeline = async (filePath: string) => {
    if (activeTimeline === filePath) { setActiveTimeline(null); setTimelineData([]); return; }
    setActiveTimeline(filePath);
    setActiveBlame(null);
    try { const result = await invoke<FileTimelineEntry[]>('get_file_timeline', { path: repoPath, filePath }); setTimelineData(result); } catch (e: any) { setError(e); }
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    const card = e.currentTarget as HTMLElement;
    const rect = card.getBoundingClientRect();
    card.style.setProperty('--torch-x', ((e.clientX - rect.left) / rect.width) * 100 + '%');
    card.style.setProperty('--torch-y', ((e.clientY - rect.top) / rect.height) * 100 + '%');
  };

  const loadHealthReport = async () => { try { const res = await invoke<HealthReport>('get_health_report', { path: repoPath }); setHealthReport(res); if (panelMode === 'replace') closeAllPanels(); setShowHealth(true); } catch (e: any) { setError(String(e)); } };
  const loadContributors = async () => { try { const res = await invoke<Contributor[]>('get_contributors', { path: repoPath }); setContributors(res); if (panelMode === 'replace') closeAllPanels(); setShowContributors(true); } catch (e: any) { setError(String(e)); } };
  const loadHotFiles = async () => { try { const res = await invoke<HotFile[]>('get_hot_files', { path: repoPath }); setHotFiles(res); if (panelMode === 'replace') closeAllPanels(); setShowHotFiles(true); } catch (e: any) { setError(String(e)); } };
  const loadStashList = async () => { try { const res = await invoke<StashEntry[]>('stash_list', { path: repoPath }); setStashList(res); if (panelMode === 'replace') closeAllPanels(); setShowStash(true); } catch (e: any) { setError(String(e)); } };
  const loadRebaseCommits = async () => { try { const res = await invoke<RebaseCommit[]>('get_rebase_commits', { path: repoPath, count: 20 }); setRebaseOps(res.map(c => ({ hash: c.hash, action: 'pick' }))); if (panelMode === 'replace') closeAllPanels(); setShowRebase(true); } catch (e: any) { setError(String(e)); } };

  const renderSectionHeader = (title: string, section: string) => (
    <div className="section-header" onClick={() => toggleSection(section)} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '8px 12px', cursor: 'pointer', userSelect: 'none' }}>
      <motion.span animate={{ rotate: expandedSections[section] ? 90 : 0 }} transition={{ duration: 0.2 }} style={{ display: 'inline-flex' }}>
        <VscChevronRight size={12} />
      </motion.span>
      <span className="section-title" style={{ marginBottom: 0 }}>{title}</span>
    </div>
  );

  // <^首启检测：localStorage not have hasSeenWelcome => yes&>
  const [showWelcome, setShowWelcome] = useState(() => {
    return localStorage.getItem('hasSeenWelcome') !== 'true';
  });

  const handleWelcomeSelect = (style: 'preset' | 'md' | 'custom') => {
    setBgStyle(style);
    localStorage.setItem('bgStyle', style);
    localStorage.setItem('hasSeenWelcome', 'true');
    setShowWelcome(false);
    // <^自 => 文件选择器&>
    if (style === 'custom') {
      // open文件选择器
      handlePickBackgroundDirect();
    }
  };

  // <^原handlePickBackground => 独立函数&>
  const handlePickBackgroundDirect = async () => {
    try {
      const b64 = await invoke<string>('pick_background_image');
      if (b64) {
        setBgBase64(b64);
        localStorage.setItem('bg_base64', b64);
        setBgStyle('custom');
        localStorage.setItem('bgStyle', 'custom');
      }
    } catch (e: any) {
      setError(e);
    }
  };

  return (
    <>
      {/* <^背景渲染：根据 bgStyle 决定显示预设图片、MD纯色或自定义图片&> */}
      {bgStyle === 'md' ? (
        <div style={{
          position: 'fixed',
          top: 0,
          right: 0,
          width: '100%',
          height: '100%',
          zIndex: -1,
          background: 'linear-gradient(145deg, #1a1a2e, #16213e, #0f3460)',
          opacity: bgOpacity,
          pointerEvents: 'none',
        }} />
      ) : (
        <img src={`data:image/jpeg;base64,${bgBase64}`} alt="background" style={{
          position: 'fixed', top: 0, right: 0, width: 'auto', height: '100%',
          objectFit: 'contain', objectPosition: 'right center', zIndex: -1, pointerEvents: 'none',
          opacity: bgOpacity
        }} />
      )}
      <div style={{ position: 'fixed', top: 0, left: 0, width: '100%', height: '100%', zIndex: -1, background: 'radial-gradient(ellipse at center, rgba(5,9,20,0.08) 0%, rgba(5,9,20,0.25) 100%)' }} />
      <motion.div className="pointer-glow" style={{ left: springX, top: springY, position: 'fixed' }} />

      <FeedbackOverlay loading={loading || detailLoading || safeDirFixing} loadingText={safeDirFixing ? '修复安全目录中...' : loading || detailLoading ? '加载中...' : null} toast={toast} onToastClose={() => setToast(null)} />
      <div className="app">
        <header className="topbar">
          <h1><SiGit size={22} color="#5B9BD5" /> GitSync</h1>
          <button
            onClick={openFolder}
            style={{ background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 10, padding: '7px 10px', color: '#c8d6e5', cursor: 'pointer', display: 'flex', alignItems: 'center' }}
            title="打开文件夹"
          >
            <VscFolderOpened size={18} />
          </button>
          <input className="path-input" type="text" value={repoPath} onChange={(e) => setRepoPath(e.target.value)} placeholder="输入仓库路径..." onKeyDown={(e) => e.key === 'Enter' && loadRepo()} />
          <div style={{ display: 'flex', gap: 8, flex: 1 }}>
            <input className="path-input" type="text" value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} placeholder="搜索提交..." onKeyDown={(e) => e.key === 'Enter' && handleSearch()} style={{ flex: 1 }} />
            <motion.button whileHover={{ scale: 1.03 }} whileTap={{ scale: 0.95 }} onClick={handleSearch} style={{ background: 'rgba(91,155,213,0.1)', border: '1px solid rgba(91,155,213,0.2)', color: '#5B9BD5', padding: '7px 12px', borderRadius: 10, cursor: 'pointer' }}>
              <VscSearch size={16} />
            </motion.button>
          </div>
          <motion.button whileHover={{ scale: 1.03 }} whileTap={{ scale: 0.95 }} onClick={(e) => { createRipple(e); loadRepo(); }} disabled={loading} style={{ position: 'relative', overflow: 'hidden' }}>
            {loading ? '加载中...' : '加载'}
            {ripples.map(r => (<span key={r.id} className="ripple" style={{ left: r.x - 20, top: r.y - 20, width: 40, height: 40 }} />))}
          </motion.button>
        </header>

        <aside className="sidebar" style={{ overflowY: 'auto', WebkitOverflowScrolling: 'touch', scrollBehavior: 'smooth' }}>
          <div className="section-title"><VscSourceControl size={12} style={{ marginRight: 6 }} />仓库信息</div>
          <div className="separator" />
          <div className="section-title"><VscRepoForked size={12} style={{ marginRight: 6 }} />分支</div>
          {branches.length > 0 ? branches.map(b => (
            <div key={b.name} className={`branch-item ${b.name === activeBranch ? 'active' : ''}`} onClick={() => switchBranch(b.name)}>
              <span className="branch-dot" style={{ background: b.name === activeBranch ? '#5B9BD5' : '#576574' }} />
              {b.name}
            </div>
          )) : (
            <div style={{ color: 'rgba(255,255,255,0.4)', fontSize: 12, padding: '0 6px' }}>加载仓库后显示</div>
          )}
          <div className="separator" />

          {renderSectionHeader('历史浏览', 'history')}
          <AnimatePresence>
            {expandedSections.history && (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowGraph(!showGraph); setScrollToPanelId('panel-graph'); }}>提交图</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowFileTree(!showFileTree); setScrollToPanelId('panel-filetree'); }}>文件树</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowCommitFilter(!showCommitFilter); setScrollToPanelId('panel-filter'); }}>提交筛选</div>
              </motion.div>
            )}
          </AnimatePresence>

          {renderSectionHeader('代码追溯', 'trace')}
          <AnimatePresence>
            {expandedSections.trace && (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowSyntaxHighlight(!showSyntaxHighlight); setScrollToPanelId('panel-syntax'); }}>Diff 高亮</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowSideBySide(!showSideBySide); setScrollToPanelId('panel-sidebyside'); }}>并排对比</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowSemanticSearch(!showSemanticSearch); setScrollToPanelId('panel-semantic'); }}>语义搜索</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowDiffViewer(!showDiffViewer); setScrollToPanelId('panel-diffviewer'); }}>差异对比</div>
              </motion.div>
            )}
          </AnimatePresence>

          {renderSectionHeader('分析工具', 'analysis')}
          <AnimatePresence>
            {expandedSections.analysis && (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
                <div className="branch-item" onClick={() => { loadHealthReport(); setScrollToPanelId('panel-health'); }}>仓库健康</div>
                <div className="branch-item" onClick={() => { loadContributors(); setScrollToPanelId('panel-contributors'); }}>贡献者统计</div>
                <div className="branch-item" onClick={() => { loadHotFiles(); setScrollToPanelId('panel-hotfiles'); }}>热点文件</div>
              </motion.div>
            )}
          </AnimatePresence>

          {renderSectionHeader('操作工具', 'actions')}
          <AnimatePresence>
            {expandedSections.actions && (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
                <div className="branch-item" onClick={() => { loadStashList(); setScrollToPanelId('panel-stash'); }}>Stash 管理</div>
                <div className="branch-item" onClick={() => { loadRebaseCommits(); setScrollToPanelId('panel-rebase'); }}>交互 Rebase</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowTagManager(!showTagManager); setScrollToPanelId('panel-tags'); }}>标签管理</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowRemoteManager(!showRemoteManager); setScrollToPanelId('panel-remotes'); }}>远程仓库</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowMultiRepo(!showMultiRepo); setScrollToPanelId('panel-multirepo'); }}>多仓库</div>
              </motion.div>
            )}
          </AnimatePresence>

          {renderSectionHeader('搜索导出', 'search_export')}
          <AnimatePresence>
            {expandedSections.search_export && (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowChangelog(!showChangelog); setScrollToPanelId('panel-changelog'); }}>变更日志</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowExportHTML(!showExportHTML); setScrollToPanelId('panel-export'); }}>导出报告</div>
              </motion.div>
            )}
          </AnimatePresence>

          {renderSectionHeader('高级功能', 'advanced')}
          <AnimatePresence>
            {expandedSections.advanced && (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: 'auto', opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }} style={{ overflow: 'hidden' }}>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowHookManager(!showHookManager); setScrollToPanelId('panel-hooks'); }}>Git Hooks</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowConflictResolver(!showConflictResolver); setScrollToPanelId('panel-conflict'); }}>冲突解决</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowScriptRunner(!showScriptRunner); setScrollToPanelId('panel-scripts'); }}>脚本扩展</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowQueryConsole(!showQueryConsole); setScrollToPanelId('panel-sql'); }}>SQL 查询</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowTimeMachine(!showTimeMachine); setScrollToPanelId('panel-timemachine'); }}>时间机器</div>
                <div className="branch-item" onClick={() => { if (panelMode === 'replace') closeAllPanels(); setShowUIManager(!showUIManager); setScrollToPanelId('panel-ui'); }}>UI 管理</div>
              </motion.div>
            )}
          </AnimatePresence>
        </aside>

        <main className="main" ref={mainRef} style={{ overflowY: 'auto', WebkitOverflowScrolling: 'touch', scrollBehavior: 'smooth' }}>
          {error && (<motion.div className="error" initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }}>{error}</motion.div>)}
          {commits.length > 0 && (<div className="status-bar"><span className="status-dot" /><VscGitCommit size={14} />{commits.length} 个提交</div>)}
          {commits.length === 0 && !error && !loading && (<div className="empty-state"><VscEmptyWindow size={48} /><p style={{ marginTop: 12 }}>输入仓库路径并加载，查看提交历史</p></div>)}
          <div className="commit-list">
            <AnimatePresence>
              {order.map((index) => {
                const c = commits[index];
                if (!c) return null;
                const isSelected = selectedCommit === c.hash;
                return (
                  <div key={c.hash} style={{ display: 'flex', flexDirection: 'column' }}>
                    <motion.div
                      className={`commit-item ${isSelected ? 'selected' : ''}`}
                      onClick={() => handleCommitClick(c.hash)}
                      onMouseMove={handleMouseMove}
                      initial={index < 20 ? { opacity: 0, x: -20 } : undefined}
                      animate={index < 20 ? { opacity: 1, x: 0 } : undefined}
                      exit={index < 20 ? { opacity: 0, x: 20 } : undefined}
                      transition={index < 20 ? { delay: Math.min(index * 0.004, 0.12), duration: 0.25, type: 'spring', stiffness: 150 } : undefined}
                      drag="y"
                      dragConstraints={dragConstraintRef}
                      dragElastic={0.2}
                      onDragEnd={(_, info) => handleDragEnd(index, info)}
                      whileHover={{ scale: 1.02, boxShadow: '0 12px 30px rgba(0,0,0,0.5)', rotateX: 1, rotateY: -1 }}
                      whileTap={{ scale: 0.98 }}
                      style={{ position: 'relative', transformStyle: 'preserve-3d' }}
                    >
                      <div className="torch-glow" />
                      <div className="commit-header"><span className="hash">{c.hash.substring(0, 8)}</span><span className="author">{c.author}</span><span className="time">{c.time}</span></div>
                      <div className="message">{c.message}</div>
                    </motion.div>

                    <AnimatePresence>
                      {isSelected && (
                        <motion.div className="commit-detail" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.3, type: 'spring', stiffness: 120 }} style={{ marginTop: 8, marginBottom: 16, overflow: 'hidden' }}>
                          {detailLoading ? (
                            <div style={{ color: 'rgba(255,255,255,0.5)', textAlign: 'center', padding: 20 }}>加载详情中...</div>
                          ) : commitDetail ? (
                            <div>
                              {/* Header controls for browsing mode */}
                              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
                                <div style={{ fontSize: 14, color: '#dfe6e9', fontWeight: 600 }}>{commitDetail.files.length} 个文件变更</div>
                                {commitDetail.files.length > 1 && (
                                  <div style={{ display: 'flex', gap: 8 }}>
                                    <button
                                      className={`btn ${viewMode === 'single' ? 'btn-blue' : ''}`}
                                      style={{ padding: '4px 10px', fontSize: 12 }}
                                      onClick={() => setViewMode('single')}
                                    >
                                      单文件滑动
                                    </button>
                                    <button
                                      className={`btn ${viewMode === 'list' ? 'btn-blue' : ''}`}
                                      style={{ padding: '4px 10px', fontSize: 12 }}
                                      onClick={() => setViewMode('list')}
                                    >
                                      列表视图
                                    </button>
                                  </div>
                                )}
                              </div>

                              {/* Draggable Progress Bar / Range Input for browsing files */}
                              {viewMode === 'single' && commitDetail.files.length > 1 && (
                                <div className="file-changes-slider-container" style={{ margin: '12px 0 20px 0', padding: '16px', background: 'rgba(255,255,255,0.03)', borderRadius: 12, border: '1px solid rgba(255,255,255,0.05)' }}>
                                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, marginBottom: 8, color: '#dfe6e9' }}>
                                    <span>拖拽进度浏览: <strong>{currentFileIndex + 1}</strong> / {commitDetail.files.length}</span>
                                    <span style={{ color: '#5B9BD5', fontWeight: 600 }}>{commitDetail.files[currentFileIndex]?.path}</span>
                                  </div>
                                  <input
                                    type="range"
                                    min={0}
                                    max={commitDetail.files.length - 1}
                                    value={currentFileIndex}
                                    onChange={(e) => setCurrentFileIndex(Number(e.target.value))}
                                    style={{ width: '100%', cursor: 'pointer' }}
                                  />
                                </div>
                              )}

                              {/* Rendering files changes */}
                              {viewMode === 'single' ? (
                                (() => {
                                  const file = commitDetail.files[currentFileIndex] || commitDetail.files[0];
                                  if (!file) return null;
                                  return (
                                    <div key={file.path} className="detail-file">
                                      <div className="detail-file-header">
                                        <span className={`file-status file-status-${file.status}`}>{file.status}</span>
                                        <span className="file-path">{file.path}</span>
                                        <span style={{ marginLeft: 'auto', fontSize: 12, display: 'flex', gap: 8 }}>
                                          <span style={{ color: '#4fc1ff' }}><VscDiffAdded size={12} /> {file.additions}</span>
                                          <span style={{ color: '#ff6b6b' }}><VscDiffRemoved size={12} /> {file.deletions}</span>
                                          <button className="detail-action-btn" onClick={(e) => { e.stopPropagation(); handleBlame(file.path); }} title="查看 Blame"><VscFileCode size={14} /></button>
                                          <button className="detail-action-btn" onClick={(e) => { e.stopPropagation(); handleTimeline(file.path); }} title="文件时间线"><VscHistory size={14} /></button>
                                        </span>
                                      </div>
                                      
                                      {/* Code Syntax Highlighted Diff */}
                                      {parseAndRenderDiff(file.diff)}

                                      <AnimatePresence>
                                        {activeBlame === file.path && (
                                          <motion.div className="blame-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}>
                                            <div className="blame-header">Blame — {file.path}</div>
                                            {blameData.map((line) => (
                                              <div key={line.line_number} className="blame-line">
                                                <span className="blame-hash">{line.commit_hash.substring(0, 8)}</span><span className="blame-author">{line.author}</span><span className="blame-time">{line.time}</span><span className="blame-content">{line.content}</span>
                                              </div>
                                            ))}
                                          </motion.div>
                                        )}
                                      </AnimatePresence>
                                      <AnimatePresence>
                                        {activeTimeline === file.path && (
                                          <motion.div className="timeline-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}>
                                            <div className="timeline-header">文件历史 — {file.path}</div>
                                            {timelineData.map((entry) => (
                                              <div key={entry.commit_hash} className="timeline-entry">
                                                <div className="timeline-entry-header"><span className="hash">{entry.commit_hash.substring(0, 8)}</span><span className="author">{entry.author}</span><span className="time">{entry.time}</span></div>
                                                <div className="message">{entry.message}</div>
                                                {/* Code Syntax Highlighted Diff */}
                                                {parseAndRenderDiff(entry.diff)}
                                              </div>
                                            ))}
                                          </motion.div>
                                        )}
                                      </AnimatePresence>
                                    </div>
                                  );
                                })()
                              ) : (
                                commitDetail.files.map((file) => (
                                  <div key={file.path} className="detail-file">
                                    <div className="detail-file-header">
                                      <span className={`file-status file-status-${file.status}`}>{file.status}</span>
                                      <span className="file-path">{file.path}</span>
                                      <span style={{ marginLeft: 'auto', fontSize: 12, display: 'flex', gap: 8 }}>
                                        <span style={{ color: '#4fc1ff' }}><VscDiffAdded size={12} /> {file.additions}</span>
                                        <span style={{ color: '#ff6b6b' }}><VscDiffRemoved size={12} /> {file.deletions}</span>
                                        <button className="detail-action-btn" onClick={(e) => { e.stopPropagation(); handleBlame(file.path); }} title="查看 Blame"><VscFileCode size={14} /></button>
                                        <button className="detail-action-btn" onClick={(e) => { e.stopPropagation(); handleTimeline(file.path); }} title="文件时间线"><VscHistory size={14} /></button>
                                      </span>
                                    </div>
                                    
                                    {/* Code Syntax Highlighted Diff */}
                                    {parseAndRenderDiff(file.diff)}

                                    <AnimatePresence>
                                      {activeBlame === file.path && (
                                        <motion.div className="blame-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}>
                                          <div className="blame-header">Blame — {file.path}</div>
                                          {blameData.map((line) => (
                                            <div key={line.line_number} className="blame-line">
                                              <span className="blame-hash">{line.commit_hash.substring(0, 8)}</span><span className="blame-author">{line.author}</span><span className="blame-time">{line.time}</span><span className="blame-content">{line.content}</span>
                                            </div>
                                          ))}
                                        </motion.div>
                                      )}
                                    </AnimatePresence>
                                    <AnimatePresence>
                                      {activeTimeline === file.path && (
                                        <motion.div className="timeline-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}>
                                          <div className="timeline-header">文件历史 — {file.path}</div>
                                          {timelineData.map((entry) => (
                                            <div key={entry.commit_hash} className="timeline-entry">
                                              <div className="timeline-entry-header"><span className="hash">{entry.commit_hash.substring(0, 8)}</span><span className="author">{entry.author}</span><span className="time">{entry.time}</span></div>
                                              <div className="message">{entry.message}</div>
                                              {/* Code Syntax Highlighted Diff */}
                                              {parseAndRenderDiff(entry.diff)}
                                            </div>
                                          ))}
                                        </motion.div>
                                      )}
                                    </AnimatePresence>
                                  </div>
                                ))
                              )}
                            </div>
                          ) : (
                            <div style={{ color: 'rgba(255,255,255,0.5)', textAlign: 'center', padding: 20 }}>无法加载详情</div>
                          )}
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>
                );
              })}
            </AnimatePresence>
          </div>

          {showHealth && healthReport && (
            <motion.div id="panel-health" className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
              <h3>仓库健康报告</h3>
              {healthReport.large_files.length > 0 && (<div className="analysis-section"><div className="section-title">大文件</div>{healthReport.large_files.map(f => <div key={f} className="analysis-item">{f}</div>)}</div>)}
              {healthReport.stale_branches.length > 0 && (<div className="analysis-section"><div className="section-title">无上游分支</div>{healthReport.stale_branches.map(b => <div key={b} className="analysis-item">{b}</div>)}</div>)}
              {healthReport.conflicts.length > 0 && (<div className="analysis-section"><div className="section-title">合并冲突文件</div>{healthReport.conflicts.map(c => <div key={c} className="analysis-item">{c}</div>)}</div>)}
              {healthReport.large_files.length === 0 && healthReport.stale_branches.length === 0 && healthReport.conflicts.length === 0 && (<div className="analysis-item">没有发现需要关注的问题</div>)}
            </motion.div>
          )}

          {showContributors && (
            <motion.div id="panel-contributors" className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
              <h3>贡献者统计</h3>
              {contributors.map(c => (<div key={c.author} className="analysis-item"><span className="hash">{c.author}</span><span className="time" style={{ float: 'right' }}>提交 {c.commits} 次，新增 {c.additions} 行，删除 {c.deletions} 行</span></div>))}
            </motion.div>
          )}

          {showHotFiles && (
            <motion.div id="panel-hotfiles" className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
              <h3>热点文件 Top 20</h3>
              {hotFiles.map(f => (<div key={f.path} className="analysis-item"><span className="file-path">{f.path}</span><span className="time" style={{ float: 'right' }}>{f.changes} 次变更</span></div>))}
            </motion.div>
          )}

          {showStash && (
            <motion.div id="panel-stash" className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
              <h3>Stash 列表</h3>
              {stashList.length === 0 && <div className="analysis-item">暂无 stash</div>}
              {stashList.map(s => (
                <div key={s.index} className="analysis-item">
                  <span className="hash">stash@{`{${s.index}}`}</span><span className="message">{s.message}</span>
                  <button className="detail-action-btn" onClick={async () => { await invoke('stash_pop', { path: repoPath, index: s.index }); loadStashList(); }}>pop</button>
                  <button className="detail-action-btn" onClick={async () => { await invoke('stash_drop', { path: repoPath, index: s.index }); loadStashList(); }}>drop</button>
                </div>
              ))}
              <button className="btn btn-blue" style={{ marginTop: 10 }} onClick={async () => { await invoke('stash_save', { path: repoPath, message: null }); loadStashList(); }}>保存当前改动</button>
            </motion.div>
          )}

          {showRebase && (
            <motion.div id="panel-rebase" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} style={{ marginTop: 16 }}>
              <EnhancedRebase repoPath={repoPath} onComplete={() => { setShowRebase(false); loadRepo(); }} />
            </motion.div>
          )}

          {showGraph && <div id="panel-graph"><GraphView repoPath={repoPath} onSelectCommit={(hash) => handleCommitClick(hash)} /></div>}
          {showFileTree && (
            <div id="panel-filetree" style={{ display: 'flex', gap: 16, alignItems: 'flex-start', flexWrap: 'wrap', width: '100%' }}>
              <div style={{ flex: '1 1 300px', minWidth: 280 }}>
                <FileTree repoPath={repoPath} onSelectFile={(path) => setSelectedTreeFile(path)} />
              </div>
              {selectedTreeFile && (
                <div style={{ flex: '2 1 500px', minWidth: 320 }}>
                  <FileViewer repoPath={repoPath} filePath={selectedTreeFile} onClose={() => setSelectedTreeFile(null)} />
                </div>
              )}
            </div>
          )}
          {showCommitFilter && <div id="panel-filter"><CommitFilter repoPath={repoPath} onFiltered={(commits) => setCommits(commits)} /></div>}
          {showTagManager && <div id="panel-tags"><TagManager repoPath={repoPath} /></div>}
          {showRemoteManager && <div id="panel-remotes"><RemoteManager repoPath={repoPath} /></div>}
          {showMultiRepo && <div id="panel-multirepo"><MultiRepo onSelectRepo={(path) => setRepoPath(path)} /></div>}
          {showSemanticSearch && <div id="panel-semantic"><SemanticSearch repoPath={repoPath} /></div>}
          {showDiffViewer && <div id="panel-diffviewer"><DiffViewer repoPath={repoPath} /></div>}
          {showChangelog && <div id="panel-changelog"><ChangelogGenerator repoPath={repoPath} /></div>}
          {showSyntaxHighlight && <div id="panel-syntax"><SyntaxHighlight repoPath={repoPath} commitHash={selectedCommit || ''} /></div>}
          {showSideBySide && <div id="panel-sidebyside"><SideBySideDiff repoPath={repoPath} commitHash={selectedCommit || ''} /></div>}
          {showHookManager && <div id="panel-hooks"><HookManager repoPath={repoPath} /></div>}
          {showConflictResolver && <div id="panel-conflict"><ConflictResolver repoPath={repoPath} /></div>}
          {showExportHTML && <div id="panel-export"><ExportHTML repoPath={repoPath} /></div>}
          {showScriptRunner && <div id="panel-scripts"><ScriptRunner repoPath={repoPath} /></div>}
          {showQueryConsole && <div id="panel-sql"><QueryConsole repoPath={repoPath} /></div>}
          {showTimeMachine && <div id="panel-timemachine"><TimeMachine repoPath={repoPath} /></div>}
          {showUIManager && <div id="panel-ui"><UIManager
            bgOpacity={bgOpacity} setBgOpacity={setBgOpacity}
            setBgBase64={setBgBase64}
            defaultBgBase64={DEFAULT_BG_BASE64}
            panelMode={panelMode} setPanelMode={setPanelMode}
            theme={theme} setTheme={setTheme}
            torchSize={torchSize} setTorchSize={setTorchSize}
            bgStyle={bgStyle} setBgStyle={setBgStyle}
            onPickCustomBackground={handlePickBackgroundDirect}
            bgBase64={bgBase64}
          /></div>}
        </main>
      </div>

      <CommandPalette commands={[
        { id: 'health', label: '仓库健康报告', action: () => { setShowHealth(true); loadHealthReport(); } },
        { id: 'contributors', label: '贡献者统计', action: () => { setShowContributors(true); loadContributors(); } },
        { id: 'hotfiles', label: '热点文件', action: () => { setShowHotFiles(true); loadHotFiles(); } },
        { id: 'stash', label: 'Stash 管理', action: () => { setShowStash(true); loadStashList(); } },
        { id: 'rebase', label: '交互 Rebase', action: () => loadRebaseCommits() },
        { id: 'search', label: '语义搜索', action: () => setShowSemanticSearch(true) },
        { id: 'diff', label: '差异对比', action: () => setShowDiffViewer(true) },
        { id: 'changelog', label: '生成变更日志', action: () => setShowChangelog(true) },
        { id: 'graph', label: '提交图', action: () => setShowGraph(true) },
        { id: 'filetree', label: '文件树', action: () => setShowFileTree(true) },
        { id: 'filter', label: '提交筛选', action: () => setShowCommitFilter(true) },
        { id: 'tags', label: '标签管理', action: () => setShowTagManager(true) },
        { id: 'remotes', label: '远程仓库', action: () => setShowRemoteManager(true) },
        { id: 'multirepo', label: '多仓库', action: () => setShowMultiRepo(true) },
        { id: 'syntax', label: 'Diff 高亮', action: () => setShowSyntaxHighlight(true) },
        { id: 'sidebyside', label: '并排对比', action: () => setShowSideBySide(true) },
        { id: 'hooks', label: 'Git Hooks', action: () => setShowHookManager(true) },
        { id: 'conflict', label: '冲突解决', action: () => setShowConflictResolver(true) },
        { id: 'exporthtml', label: '导出 HTML 报告', action: () => setShowExportHTML(true) },
        { id: 'scripts', label: '脚本扩展', action: () => setShowScriptRunner(true) },
        { id: 'sql', label: 'SQL 查询', action: () => setShowQueryConsole(true) },
        { id: 'timemachine', label: '时间机器', action: () => setShowTimeMachine(true) },
        { id: 'uimanager', label: 'UI 管理', action: () => setShowUIManager(true) },
      ]} />

      <AnimatePresence>
        {safeDirModalPath && (
          <SafeDirectoryModal
            path={safeDirModalPath}
            fixing={safeDirFixing}
            error={safeDirFixingError}
            onClose={() => setSafeDirModalPath(null)}
            onFix={handleFixSafeDirectory}
          />
        )}
      </AnimatePresence>

      {/* <^first open welcoming&> */}
      <WelcomeModal
        isOpen={showWelcome}
        onSelect={handleWelcomeSelect}
        onClose={() => {
          // <^关闭弹窗without选择 => 使用 preset&>
          setBgStyle('preset');
          localStorage.setItem('bgStyle', 'preset');
          localStorage.setItem('hasSeenWelcome', 'true');
          setShowWelcome(false);
        }}
        defaultBgBase64={DEFAULT_BG_BASE64}
        bgBase64={bgBase64}
      />
    </>
  );
}

export default App;
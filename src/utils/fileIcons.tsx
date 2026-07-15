import { 
  SiJavascript, 
  SiTypescript, 
  SiReact, 
  SiRust, 
  SiPython, 
  SiGo, 
  SiHtml5, 
  SiCss, 
  SiDocker, 
  SiPhp,
  SiRuby,
  SiSwift,
  SiYaml
} from 'react-icons/si';
import { FaJava } from 'react-icons/fa';
import { 
  VscJson, 
  VscMarkdown, 
  VscFilePdf, 
  VscFileZip, 
  VscFileMedia, 
  VscSettings, 
  VscTerminal,
  VscFileCode
} from 'react-icons/vsc';

export function getFileIcon(fileName: string, size = 16) {
  const ext = fileName.split('.').pop()?.toLowerCase() || '';
  
  // Specific file names
  if (fileName === 'package.json') return <VscJson size={size} style={{ color: '#cbcb41' }} />;
  if (fileName === 'Cargo.toml') return <VscSettings size={size} style={{ color: '#e05a47' }} />;
  if (fileName === 'tsconfig.json') return <VscJson size={size} style={{ color: '#3178c6' }} />;
  if (fileName === 'vite.config.ts' || fileName === 'vite.config.js') return <SiReact size={size} style={{ color: '#ffd600' }} />;
  if (fileName.startsWith('.git')) return <VscSettings size={size} style={{ color: '#f03c2e' }} />;
  if (fileName === 'Dockerfile' || fileName === 'docker-compose.yml') return <SiDocker size={size} style={{ color: '#0db7ed' }} />;

  // Extensions
  switch (ext) {
    case 'js':
    case 'mjs':
    case 'cjs':
      return <SiJavascript size={size} style={{ color: '#f7df1e' }} />;
    case 'ts':
    case 'mts':
      return <SiTypescript size={size} style={{ color: '#3178c6' }} />;
    case 'jsx':
      return <SiReact size={size} style={{ color: '#4fa3e3' }} />;
    case 'tsx':
      return <SiReact size={size} style={{ color: '#00d8ff' }} />;
    case 'rs':
      return <SiRust size={size} style={{ color: '#dea584' }} />;
    case 'py':
      return <SiPython size={size} style={{ color: '#3776ab' }} />;
    case 'go':
      return <SiGo size={size} style={{ color: '#00add8' }} />;
    case 'html':
    case 'htm':
      return <SiHtml5 size={size} style={{ color: '#e34f26' }} />;
    case 'css':
    case 'scss':
    case 'less':
      return <SiCss size={size} style={{ color: '#1572b6' }} />;
    case 'json':
    case 'lock':
      return <VscJson size={size} style={{ color: '#cbcb41' }} />;
    case 'yaml':
    case 'yml':
      return <SiYaml size={size} style={{ color: '#cb171e' }} />;
    case 'md':
      return <VscMarkdown size={size} style={{ color: '#083fa6' }} />;
    case 'pdf':
      return <VscFilePdf size={size} style={{ color: '#e05a47' }} />;
    case 'sh':
    case 'bash':
    case 'bat':
    case 'cmd':
    case 'ps1':
      return <VscTerminal size={size} style={{ color: '#4caf50' }} />;
    case 'zip':
    case 'tar':
    case 'gz':
    case 'rar':
    case '7z':
      return <VscFileZip size={size} style={{ color: '#eca712' }} />;
    case 'png':
    case 'jpg':
    case 'jpeg':
    case 'gif':
    case 'svg':
    case 'ico':
    case 'webp':
      return <VscFileMedia size={size} style={{ color: '#10ea92' }} />;
    case 'java':
      return <FaJava size={size} style={{ color: '#5382a1' }} />;
    case 'php':
      return <SiPhp size={size} style={{ color: '#777bb4' }} />;
    case 'rb':
      return <SiRuby size={size} style={{ color: '#cc342d' }} />;
    case 'swift':
      return <SiSwift size={size} style={{ color: '#f05138' }} />;
    case 'txt':
    case 'log':
    case 'conf':
    case 'ini':
      return <VscFileCode size={size} style={{ color: '#a2b4c6' }} />;
    default:
      // Check if it's a known config file
      if (fileName.includes('config') || fileName.startsWith('.')) {
        return <VscSettings size={size} style={{ color: '#a2b4c6' }} />;
      }
      return <VscFileCode size={size} style={{ color: '#c8d6e5' }} />;
  }
}

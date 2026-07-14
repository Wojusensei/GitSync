

// Helper functions for diff syntax highlighting
export function renderHighlightedLine(lineText: string) {
  const tokens: { text: string; type: string }[] = [];
  const tokenRegex = /(\/\/.*)|(".*?"|'.*?'|`.*?`)|(\b(?:const|let|var|function|return|import|export|from|default|class|interface|type|struct|fn|pub|use|impl|async|await|if|else|for|while|match|switch|case|break|continue|true|false|null|undefined|void|string|number|boolean|any|mut|use|mod|trait|static|ref|match|self|Self)\b)|(\b\d+\b)|([a-zA-Z_]\w*)|([^\s\w]+)|(\s+)/g;
  
  let match;
  while ((match = tokenRegex.exec(lineText)) !== null) {
    const [
      _full,
      comment,
      str,
      keyword,
      number,
      identifier,
      symbol,
      whitespace
    ] = match;
    
    if (comment) {
      tokens.push({ text: comment, type: 'comment' });
    } else if (str) {
      tokens.push({ text: str, type: 'string' });
    } else if (keyword) {
      tokens.push({ text: keyword, type: 'keyword' });
    } else if (number) {
      tokens.push({ text: number, type: 'number' });
    } else if (identifier) {
      tokens.push({ text: identifier, type: 'identifier' });
    } else if (symbol) {
      tokens.push({ text: symbol, type: 'symbol' });
    } else if (whitespace) {
      tokens.push({ text: whitespace, type: 'whitespace' });
    }
  }
  
  if (tokens.length === 0) {
    return <span>{lineText}</span>;
  }
  
  return (
    <>
      {tokens.map((token, index) => {
        let className = '';
        if (token.type === 'comment') className = 'diff-syntax-comment';
        else if (token.type === 'string') className = 'diff-syntax-string';
        else if (token.type === 'keyword') className = 'diff-syntax-keyword';
        else if (token.type === 'number') className = 'diff-syntax-number';
        else if (token.type === 'symbol') className = 'diff-syntax-symbol';
        
        return className ? (
          <span key={index} className={className}>{token.text}</span>
        ) : (
          token.text
        );
      })}
    </>
  );
}

export function parseAndRenderDiff(diffText: string) {
  if (!diffText) return null;
  const lines = diffText.split('\n');
  return (
    <div className="diff-container">
      {lines.map((line, index) => {
        if (index === lines.length - 1 && line === '') return null;
        
        // 跳过文件头行 (--- a/ 和 +++ b/)
        if (line.startsWith('--- ') || line.startsWith('+++ ')) {
          return null;
        }
        
        // 跳过其他 git diff 头部行
        if (line.startsWith('diff --git') || line.startsWith('index ') || line.startsWith('new file') || line.startsWith('deleted file')) {
          return null;
        }
        
        if (line.startsWith('+')) {
          const code = line.substring(1);
          return (
            <div key={index} className="diff-line diff-line-addition">
              <span className="diff-line-prefix">+</span>
              <span className="diff-line-code">{renderHighlightedLine(code)}</span>
            </div>
          );
        } else if (line.startsWith('-')) {
          const code = line.substring(1);
          return (
            <div key={index} className="diff-line diff-line-deletion">
              <span className="diff-line-prefix">-</span>
              <span className="diff-line-code">{renderHighlightedLine(code)}</span>
            </div>
          );
        } else if (line.startsWith('@@')) {
          return (
            <div key={index} className="diff-line diff-line-hunk">
              <span className="diff-line-prefix"> </span>
              <span className="diff-line-code">{line}</span>
            </div>
          );
        } else if (line.startsWith(' ')) {
          const code = line.substring(1);
          return (
            <div key={index} className="diff-line diff-line-context">
              <span className="diff-line-prefix"> </span>
              <span className="diff-line-code">{renderHighlightedLine(code)}</span>
            </div>
          );
        } else {
          // 未分类的行，可能是其他 git diff 格式 of the line, skip
          return null;
        }
      })}
    </div>
  );
}

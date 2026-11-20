// 会话虚拟路径域逻辑（前端镜像，服务端 kernel path.rs 权威）。
// NFC 归一化（String.normalize）+ 上限校验镜像 + trie 构建（侧栏目录树渲染）。

/** 上限常量（镜像 kernel path.rs，协议级）。 */
export const MAX_DEPTH = 8;
export const MAX_SEGMENT_CHARS = 64;
export const MAX_PATH_CHARS = 1024;

/** 归一化 + 校验虚拟路径（镜像服务端；前端预校验 UX，服务端权威）。失败抛 Error。 */
export function normalizePath(input: string): string {
  const nfc = input.normalize('NFC').trim();
  const body = nfc.replace(/^\/+/, '').replace(/\/+$/, '');
  if (body === '') throw new Error('路径不可为空（未分类请用 null）');
  const segments = body.split('/');
  if (segments.length > MAX_DEPTH) throw new Error(`嵌套深度超上限 ${MAX_DEPTH}`);
  for (const seg of segments) {
    if (seg === '') throw new Error('含空段（连续斜杠）');
    if (seg === '.' || seg === '..') throw new Error('段不可为 . 或 ..');
    if ([...seg].length > MAX_SEGMENT_CHARS) throw new Error(`段长度超上限 ${MAX_SEGMENT_CHARS} 字符`);
    if ([...seg].some((c) => {
      const code = c.charCodeAt(0);
      // C0 + DEL + C1（镜像 Rust char::is_control 的 Cc 类别）。
      return code < 32 || code === 127 || (code >= 0x80 && code <= 0x9f);
    })) {
      throw new Error('段含控制字符');
    }
    if (seg.trim() !== seg) throw new Error('段首尾不可有空白');
  }
  const path = `/${segments.join('/')}`;
  if ([...path].length > MAX_PATH_CHARS) throw new Error(`全路径超上限 ${MAX_PATH_CHARS} 字符`);
  return path;
}

/** 目录树节点。 */
export interface TreeNode {
  name: string;
  path: string;
  children: TreeNode[];
}

/** 由路径集构建 trie（侧栏目录树）。路径须已归一化。子节点按名排序。 */
export function buildTree(paths: readonly string[]): TreeNode[] {
  const root: TreeNode = { name: '', path: '', children: [] };
  for (const path of paths) {
    const segments = path.replace(/^\/+/, '').split('/').filter((s) => s !== '');
    let node = root;
    let acc = '';
    for (const seg of segments) {
      acc = `${acc}/${seg}`;
      let child = node.children.find((c) => c.name === seg);
      if (!child) {
        child = { name: seg, path: acc, children: [] };
        node.children.push(child);
      }
      node = child;
    }
  }
  const sortRec = (nodes: TreeNode[]): TreeNode[] => {
    nodes.sort((a, b) => a.name.localeCompare(b.name));
    for (const n of nodes) sortRec(n.children);
    return nodes;
  };
  return sortRec(root.children);
}

/** path 是否在 prefix 子树内（含自身，按段边界）。 */
export function isWithin(prefix: string, path: string): boolean {
  return path === prefix || path.startsWith(`${prefix}/`);
}
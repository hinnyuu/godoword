import { describe, it, expect } from 'vitest';
import { normalizePath, buildTree, isWithin, MAX_DEPTH } from '../../src/session/path';

describe('session/path', () => {
  it('归一化基本', () => {
    expect(normalizePath('/日语/五十音')).toBe('/日语/五十音');
    expect(normalizePath('日语/五十音/')).toBe('/日语/五十音');
    expect(normalizePath('  /a/b  ')).toBe('/a/b');
  });

  it('NFC 归一化日语浊音', () => {
    expect(normalizePath('/か\u3099')).toBe('/が');
  });

  it('拒绝非法', () => {
    expect(() => normalizePath('')).toThrow();
    expect(() => normalizePath('/')).toThrow();
    expect(() => normalizePath('/a//b')).toThrow();
    expect(() => normalizePath('/a/../b')).toThrow();
    expect(() => normalizePath('/a/b /c')).toThrow();
  });

  it('深度上限', () => {
    const deep = `/${Array(MAX_DEPTH + 1).fill('a').join('/')}`;
    expect(() => normalizePath(deep)).toThrow();
    const ok = `/${Array(MAX_DEPTH).fill('a').join('/')}`;
    expect(normalizePath(ok)).toBe(ok);
  });

  it('buildTree 构建目录树', () => {
    const tree = buildTree(['/日语/五十音', '/日语/对话', '/英语']);
    expect(tree.length).toBe(2);
    const ja = tree.find((n) => n.name === '日语')!;
    expect(ja.path).toBe('/日语');
    expect(ja.children.length).toBe(2);
    expect(ja.children.map((c) => c.name)).toEqual(['五十音', '对话']); // 排序
    expect(ja.children[0]!.path).toBe('/日语/五十音');
  });

  it('isWithin 子树按段边界', () => {
    expect(isWithin('/a', '/a')).toBe(true);
    expect(isWithin('/a', '/a/b')).toBe(true);
    expect(isWithin('/a', '/ab')).toBe(false);
  });
});
// 玩法的生成素材：AI 画的图缩成的像素块，收在 assets/gameModes/<玩法>/<名字>.png；
// 剧本（开局时带进来的设置）自己的一套收在 assets/gameModes/<玩法>/<剧本>/ 下，照着剧本名找。
// 于是换玩法、换剧本就换了一套素材；没有生成的图时各玩法照旧用代码画的像素图标（pixelIcons.ts）。

const files = import.meta.glob<string>('../../../assets/gameModes/**/*.png', {
  eager: true,
  query: '?url',
  import: 'default',
})

/** 玩法素材的地址；pack 是开局设置挑出的素材包（故事模式是剧本名），包里没有这张时落回玩法通用的那张 */
export function gameModeArt(mode: string, name: string, pack?: string): string | undefined {
  if (pack) {
    const own = files[`../../../assets/gameModes/${mode}/${pack}/${name}.png`]
    if (own) return own
  }
  return files[`../../../assets/gameModes/${mode}/${name}.png`]
}

/** 素材块 18×18（16 格内容加一圈透明边，和 pixelIcons 的成图一样大），按 scale 倍显示 */
export function artImage(url: string, scale: number, className: string) {
  const img = document.createElement('img')
  img.src = url
  img.alt = ''
  img.className = className
  img.width = 18 * scale
  img.height = 18 * scale
  return img
}

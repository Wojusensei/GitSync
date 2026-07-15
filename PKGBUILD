pkgname=gitsync
pkgver=0.3.1
pkgrel=1
pkgdesc="GitSync - 交互式 Git 历史浏览器，可视化提交图、分支、差异对比、Blame 等，帮助开发者更直观地理解和管理 Git 仓库"
arch=('x86_64')
url="https://github.com/Wojusensei/GitSync"
license=('MIT')

depends=(
  'gtk3'
  'webkit2gtk-4.1'
)

source=(
  "git-tool"
  "gitsync.desktop"
  "icon.png"
)

sha256sums=('eb2ed4114050aa164aaa2234c1cb17ea6cc2ec0e25378129637d2fc32b3f61f1'
  'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
  '1f3689f6374b0553996fdc99743799216703835070145d7f0e6ec11e6280139e')

package() {
  install -Dm755 \
    "$srcdir/git-tool" \
    "$pkgdir/usr/bin/gitsync"
  install -Dm644 \
    "$srcdir/gitsync.desktop" \
    "$pkgdir/usr/share/applications/gitsync.desktop"
  install -Dm644 \
    "$srcdir/icon.png" \
    "$pkgdir/usr/share/icons/hicolor/128x128/apps/gitsync.png"
}

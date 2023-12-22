# 環境構築
- ファームウェア
```
$ cd appsrc
$ cargo build
$ cd target/thumbv6m-none-eabi/debug
$ elf2uf2 picow-regin-heater picow-regin-heater.uf2
```
- モニタグラフ
```
$ cd monitor_tool/graph
$ npm install
$ npm run dev
```
localhost:3000 にアクセス

回路図、回路設計は準備中

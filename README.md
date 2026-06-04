<div align="center">
<pre>
  ███████╗██████╗  ██████╗  █████╗ ██╗  ██╗
  ██╔════╝██╔══██╗██╔═══██╗██╔══██╗██║ ██╔╝
  ███████╗██████╔╝██║   ██║███████║█████╔╝ 
  ╚════██║██╔═══╝ ██║   ██║██╔══██║██╔═██╗ 
  ███████║██║     ╚██████╔╝██║  ██║██║  ██╗
  ╚══════╝╚═╝      ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝
</pre>

# Spoak Backend (v0.2.5)

**Yüksek performanslı, Lock-Free ve Zero-Copy Minecraft Araçları API'ı.**  
Rust ve Axum ile geliştirildi. Devasa trafik yüklerine dayanıklı TTL tabanlı Moka önbelleği kullanır.

[![GitHub](https://img.shields.io/badge/GitHub-spoakk%2Ffrontend-181717?logo=github)](https://github.com/spoakk/frontend) [![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?logo=rust)](https://rust-lang.org)

</div>

---

## 🚀 API Uç Noktaları (Endpoints)

Tüm endpoint'ler saniyelik rate-limit'e tabi olup, sıkça kullanılan yanıtlar anında RAM'den (Moka) dönmektedir.

| Endpoint | İşlev |
|----------|----------|
| `GET /api/mcping` | Hedef Minecraft sunucusunun canlı verilerini, gecikme sürelerini ve MOTD formatlarını getirir. (Ping Flood Korumalı) |
| `GET /api/player/:username` | Oyuncu UUID'sini, Skin model tipini ve cape (pelerin) detaylarını Mojang'dan çekip önbellekler. |
| `GET /api/seedmap/tile` | Cubiomes C kütüphanesini kullanarak Minecraft dünyalarının anlık biyom chunk'larını üretir ve Zero-Copy (BytesMut) ile direkt bytestream döner. |
| `GET /api/seedmap/structures` | Belirli bir koordinatın etrafındaki Tapınak, Köy veya Gemi Enkazı gibi yapıları listeler. |
| `GET /api/serverjars/*` | Mojang, Paper ve Leaf build versiyonlarının güncel dağıtımlarını listeler. |

---

## 🛠️ Kurulum ve Çalıştırma

Gereksinimler: **Rust Stable (1.75+)** ve Cubiomes (C-FFI) derlemek için **GCC/Clang**.

```bash
# 1. Depoyu indirin
git clone https://github.com/spoakk/backend
cd spoak-backend

# 2. Çevresel değişkenleri yapılandırın (PORT vs.)
cp .env.example .env

# 3. Yüksek performans modunda (release) çalıştırın
cargo run --release
```

## ⚡ Optimizasyonlar
- **Zero-Copy & Moka Cache:** Ağır CPU kullanan C FFI (Cubiomes) sonuçları Moka üzerinde asenkron olarak önbelleklenir. Chunk başına $<1ms$ dönüş sağlanır.
- **Lock-Free Rate Limiter:** Arka planda sunucuyu bloke eden eski `DashMap` yapıları kaldırılarak TTL tabanlı Moka Rate-Limiter sistemine geçilmiştir.
- **Graceful Shutdown:** `ctrl_c` dinlenerek, aktif bağlantıların güvenle kapatılması sağlanır.

## 🔗 İlgili Bağlantılar
- **Web Sitesi:** [spoak.cc](https://spoak.cc)
- **Topluluk:** [Discord Sunucumuz](https://discord.gg/SBbU3rCtGe)
- **CLI Versiyonu:** [spoak-cli](https://github.com/spoakk/cli)

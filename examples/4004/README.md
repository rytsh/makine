# 4004 aritmetik / kontrol örneği

Uygulamada **Dünya → Örnek projeler → 4004 atölyesi aç**.
Örnek duraklatılmış açılır; **Saat çevrimi** bir komut yürütür.

`count.hex`, varsayılan programdır. Program ROM'unu seçip içeriğini
değiştirebilir, **İçeriği yükle** ile uygulayabilir ve **Baştan** ile yeniden
başlatabilirsin. Onaltılık sayılar; `;` yorum başlatır.

Çekirdeğin hesaplama ve komut çözmesi NAND kapılarındadır. Yalnızca
yazmaçlar ve ROM veri saklayan hazır parçalardır. Modülleri açarak ağı
görebilirsin; kütüphaneye özel bir 4004 emülatörü eklenmez.

`ram.hex`, **4004 + RAM** düğmesinin açtığı programdır: 4002 RAM'inde sayar,
ROM girişini ekler ve RAM portuna yazar (SRC, RDM, WRM, RDR, ADM, WMP).

**Sınır:** bu bir tam 4004/MCS-4 uygulaması değildir. SRC, DCL ve `Ex`
bellek/GÇ komutları vardır; ama 4002 RAM'i tek düz bir bellektir, 4001 ROM
yongaları yoktur ve WPM (program RAM'i) desteklenmez. Saat ve dış bus
sadeleştirilmiştir. Desteklenmeyen
komutta HATA=1 olur ve PC ilerlemez. [Kapsam, mimari ve deneyler](../../docs/wiki/4004.md).

servers-add-title = Medya sunucusu ekle
servers-on-network = Ağınızda
servers-looking = Sunucular aranıyor…
servers-none-found = Bu ağda sunucu bulunamadı. Adresi kendiniz yazın.
servers-address = Sunucu adresi
servers-address-hint = jellyfin.home.example ya da 192.168.1.20:8096
servers-address-note = Jellyfin'i tarayıcıda açarken kullandığınız sunucu adresini yazın.
servers-continue = { -proceed-cap }
servers-sign-in-title = { $server } sunucusunda oturum aç
servers-user = Kullanıcı adı
servers-password = Parola
servers-use-quick-connect = Bunun yerine Quick Connect kullan
servers-use-password = Bunun yerine parolayla oturum aç
servers-token-note = Parolanız kaydedilmez. { -app-name }, oturum belirtecini { $store } içinde tutar.
secure-store-windows = Kimlik bilgileri yöneticisi
secure-store-linux = anahtarlığınız
secure-store-device = Anahtar Zinciriniz
secure-store-android = bu { -device }ın güvenli depolaması
servers-sign-in = Oturum aç
servers-qc-title = Bu kodu Jellyfin'e girin
servers-qc-steps = Oturum açmış bir { -device } üzerinde Jellyfin'i açın, profilinize gidin, Quick Connect'i seçin ve bu kodu girin.
servers-qc-waiting = Onaylamanız bekleniyor
servers-qc-expired = Bu kodun süresi doldu.
servers-qc-new-code = Yeni kod al
servers-libraries-title = Kitaplıkları seçin
servers-libraries-body = Diziler AniList ile eşleştirilir, böylece Ana sayfaya, listelerinize ve Arama'ya katılırlar.
servers-library-line = { $kind } · { $count } dizi
servers-report = İzlediklerinizi Jellyfin'e bildirin
servers-report-line = Kaldığınız yerler ve izlendi işaretleri burada oynattıklarınızı izler.
servers-add = Sunucu ekle
servers-row-matching = Eşleştiriliyor · { $done } / { $total } dizi
servers-row-summary = { $libraries ->
    [one] 1 { -library } · { $series } dizi
   *[other] { $libraries } { -library } · { $series } dizi
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
   *[other] { $count } { -library }
 }
servers-row-series = { $count ->
    [one] 1 dizi
   *[other] { $count } dizi
 }
servers-row-needs-look = { $count } tanesine bakmak gerek
servers-page-line = { $address } · Jellyfin { $version } · { $user } olarak oturum açıldı
servers-sign-out = Oturumu kapat
servers-remove = Sunucuyu kaldır
servers-error-unreachable = { $address } adresine ulaşılamıyor. Adresi ve sunucunun açık olduğunu kontrol edin.
servers-error-not-jellyfin = Bu adres yanıt veriyor ama bir Jellyfin sunucusu değil.
servers-libraries = Kitaplıklar
servers-remove-line = Sunucuda hiçbir şey silinmez.
servers-removed = { $name } kaldırıldı
servers-error-too-old = Bu sunucu Jellyfin { $version } çalıştırıyor. { -app-name } için 10.8 ya da daha yenisi gerekir.
servers-error-credentials = Kullanıcı adı ya da parola yanlış.
servers-error-qc-off = Bu sunucuda Quick Connect kapalı. Parolayla oturum açın ya da Jellyfin'in panosundan açın.
servers-error-plain-http = Bu adres ağınızda değil ve HTTPS kullanmıyor, bu yüzden parolanız korumasız gider. https:// ile başlayan bir adres kullanın.
servers-error-bad-address = Bu bir adrese benzemiyor. Jellyfin'i tarayıcıda açtığınız gibi yazın.
servers-error-other = Bir şeyler ters gitti. Yeniden deneyin.
servers-cert-title = Bu sunucuyu kontrol edin
servers-cert-body = { $host }, bu { -device }ın güvenmediği bir sertifika kullanıyor. Evdeki bir sunucu için bu normaldir. Yalnızca bu parmak izi sunucunuzun gösterdiğiyle aynıysa izin verin.
servers-cert-allow = Bu sertifikaya izin ver
servers-row-signed-out = Sunucu oturumunuzu kapattı
servers-sign-in-again = Yeniden oturum aç
servers-row-not-answering = Yanıt vermiyor · son görülme { $time }
title-server-not-answering = { $server } yanıt vermiyor. İndirdiğiniz bölümler yine de oynar.
servers-kind-shows = Diziler
servers-kind-movies = Filmler
servers-kind-other = Videolar
servers-quick-connect = Quick Connect
servers-libraries-heading = Kitaplıklar
servers-refresh = Yeniden eşleştir
servers-needs-look = { $count } tanesi AniList ile eşleşmedi
servers-kind-films = Filmler
servers-error-certificate = { $host }, bu { -device }ın güvenmediği bir sertifika kullanıyor. Güvendiği bir sertifikaya sahip, https:// ile başlayan bir adres kullanın.
servers-row-signed-out-here = Oturum kapalı
servers-sign-out-line = Bu { -device }ın sunucudaki oturumunu sonlandırır. Sunucu ve indirmeleriniz olduğu gibi kalır.

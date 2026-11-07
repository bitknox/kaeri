store-joined = { $store }: { $member } olarak katıldınız.
store-joined-unnamed = { $store }: isim olmadan katıldınız.
store-left = { $store } bırakıldı. Kurulu { -plugin-pl } kalır.
plugin-removed = { $plugin } kaldırıldı.
store-member-removed = Kaldırıldı. Bu üyeye ait { -device-pl } artık { -plugin-store }nda çalışmıyor.
store-invite-cancelled = Davet iptal edildi. Bağlantı artık çalışmıyor.
store-invite-cancelled-kept = { $count ->
    [one] Davet iptal edildi. Katılan kişi üye olarak kalır.
   *[other] Davet iptal edildi. Katılan { $count } kişi üye olarak kalır.
 }
store-open-join-on = Bu { -plugin-store }nın bağlantısına sahip herkes artık katılabilir. Bağlantıyı üye ekranından kopyalayın.
store-open-join-off = Artık yalnızca davet ettikleriniz katılabilir.
store-open-join-off-devices = { $devices ->
    [one] Artık yalnızca davet ettikleriniz katılabilir. Bağlantıyla katılan { -device } siz kaldırana kadar çalışmaya devam eder.
   *[other] Artık yalnızca davet ettikleriniz katılabilir. Bağlantıyla katılan { $devices } { -device-pl } siz kaldırana kadar çalışmaya devam eder.
 }
store-device-removed = Bu { -device } artık bu { -plugin-store }nda çalışmıyor.
store-devices-removed = Bağlantıyla katılan bütün { -device-pl } kaldırıldı.
plugin-published = { $plugin } { $version } yayımlandı.
plugin-unpublished = { $plugin } artık sunulmuyor.
store-admin-opened = Yönetici sayfası tarayıcınızda açıldı. Bağlantı yalnızca bir kez ve 10 dakika çalışır.
store-admin-open-failed = Tarayıcı açılamadı ({ $reason }). Bağlantı yazdırmak için { -plugin-store }nın bulunduğu makinede "kaeri store admin" komutunu çalıştırın.
plugin-installed-unused = { $plugin } { $version } kuruldu, ama { -plugin-pl } { -source } sıralamasında kapalı olduğundan kullanılmıyor.
plugin-installed-name-clash = { $plugin } kuruldu, ama { $id } yerleşik bir { -source } adı olduğu için yüklenmedi.
plugin-installed = { $plugin } { $version } kuruldu.
plugin-installed-not-loaded = { $plugin } kuruldu, ama yüklenemedi. { $reason }
plugin-installed-outdated = { $plugin } kuruldu, ancak { -app-name } uygulamasının eski bir sürümü için derlenmiş ve güncellenmesi gerekiyor. Yapımcısından yeni bir derleme isteyin.
plugins-outdated = { $count ->
    [one] Bir { -plugin }, { -app-name } uygulamasının eski bir sürümü için derlenmiş ve güncellenmesi gerekiyor: { $names }.
   *[other] { $count } { -plugin-pl }, { -app-name } uygulamasının eski bir sürümü için derlenmiş ve güncellenmesi gerekiyor: { $names }.
 }
plugin-updates-ready = { $count ->
    [one] { -plugin-cap } güncellemesi hazır: { $names }.
   *[other] { -plugin-cap } güncellemeler hazır: { $names }.
 }
plugins-load-failed = { -plugin-cap-pl } yüklenemedi. { $reason }
store-token-missing = { $store } için bu { -device }da kayıtlı bir belirteç yok. Yeni bir davetle yeniden katılın.
store-not-joined = Bu { -plugin-store } artık katıldıklarınız arasında değil.
store-key-damaged = Bu { -device }da { -plugin-store }nın imza anahtarı bozuk. { -plugin-store }nı bırakıp yeni bir davetle yeniden katılın.
store-device-refused = { $store } bu cihazı artık kabul etmiyor. Yeni bir davetle yeniden katılın.
store-unreachable = { $store } ile bağlantı kurulamadı. { $reason }
store-moved = { $store }, { $current } adresinde katılmış durumda, ama bu bağlantı { $target } adresini gösteriyor. Bu { -plugin-store } taşındıysa önce onu bırakıp yeniden katılın.
store-join-would-unname = { $store }: { $member } olarak katıldınız. Bu bağlantı adınız olmadan yeniden katılmanızı sağlardı, bu yüzden hiçbir şey değiştirilmedi.
store-not-the-invited = { $url } davette adı geçen { -plugin-store } değil ({ $reason }). Hiçbir şeye katılınmadı.
store-token-not-kept = Bu { -plugin-store } için belirteç kaydedilemedi. { $reason }
store-invite-unusable = Bu davet zaten kullanılmış, iptal edilmiş ya da süresi dolmuş. Yeni bir tane isteyin.
store-invite-used-up = Bu davetin kullanım hakkı bitti. { $owners } kişisinden yenisini isteyin.
store-invite-ended = Bu davet { $when } tarihinde sona erdi. { $owners } kişisinden yenisini isteyin.
store-invite-was-cancelled = Bu davet iptal edildi. { $owners } kişisinden yenisini isteyin.
store-invite-contact = İletişim: { $contact }
store-open-link-unusable = Bu bağlantı artık kimsenin katılmasına izin vermiyor. Yeni bir bağlantı ya da davet için { -plugin-store }nın sahibinden isteyin.
plugin-reserved-name = { $plugin } yerleşik bir { -source } adı, bu yüzden { -plugin } olamaz.
store-plugin-withdrawn = { $store } artık { $plugin } sunmuyor.
store-plugin-changed = { $store }, { $plugin } { -plugin }sini gösterildikten sonra değiştirdi. Kurmadan önce tekrar bakın.
plugin-from-other-store = { $plugin } { $store } üzerinden kuruldu. Güncellemeleri oradan gelir.
plugin-duplicate-file = { $path } zaten { $plugin } tanımlıyor. Önce o dosyayı kaldırın, yoksa iki kopya çalışır.
plugin-install-failed = { $plugin } kurulamadı. { $reason }
plugin-publish-failed = { $plugin } yayımlanamadı. { $reason }
plugin-missing-locally = { $plugin } artık { -plugin } klasöründe yok.
store-takes-no-books = { $store } henüz kitap { -source-pl } listelemiyor. Sahibi bunları eklemek için { -plugin-store }nı güncelleyebilir.
store-invite-needs-link = Bu { -device } bağlantıyla { $store } listesine katıldı. Aynı bağlantıyı diğer { -device }da yapıştırın.
store-link-malformed = Bu { -plugin-store }nın gönderdiği bağlantı çalışmıyor. Sahibinden { -plugin-store }nı kontrol etmesini isteyin. { $reason }
store-link-wrong-key = Bu { -plugin-store } farklı bir anahtarla bağlantı oluşturdu. Başladığınız adresi kontrol edin.
store-admin-missing = { $store } yönetici sayfasına sahip değil. Sunucusu daha eski olabilir.
store-admin-link-foreign = { $store } başka bir adresi gösteren bir yönetici bağlantısı gönderdi.
store-open-join-unsupported = { $store } sunucusu daha eski, bu yüzden bağlantıyla kimse katılamaz. Önce güncelleyin.
source-switched-off = { $name } kapatıldı. Bölüm ararken atlanır.
source-switched-on = { $name } açıldı.
plugin-not-loaded = { $id } yüklenmedi. { -source-cap-pl } altında kurulu olduğunu kontrol edin.
release-pinned = { $name } bu başlık için kullanılacak.
notice-choice-not-saved = Bu seçim kaydedilemedi. { $reason }
torrent-proxy-waiting = Proxy denetimi başarısız oldu. Torrent'ler denetim geçilince başlar.
torrent-proxy-unchecked = Torrent'ler proxy denetimi geçilince başlar.
torrent-link-unusable = Bu bir magnet bağlantısı ya da torrent dosyası değil.
torrent-link-playback-off = Kendi bağlantılarınızı oynatmak için Ayarlar'dan torrent oynatmayı açın.
opened-search-failed = Bu başlık aranamadı. { $reason }
torrent-interface-closed = { -app-name } torrentler başlamadan kapandı.
torrent-session-failed = Torrent oturumu başlatılamadı. { $reason }
torrent-proxy-unusable = Proxy ayarı kullanılamıyor. { $reason }
sources-reloaded-torrents-checking = { -source-cap-pl } yeniden yüklendi. Torrent'ler proxy denetimi geçilince başlar.
sources-reloaded-torrents-off = Torrentler kapalı. Oturum durduruldu.
job-anilist-limit = AniList istek sınırı nedeniyle bekleniyor
notice-anilist-limit = AniList istek sınırı nedeniyle { $wait } bekleniyor.
notice-anilist-unreachable = AniList yanıt vermiyor. Şimdilik başka bir hizmetten veriler gösteriliyor.
notice-metadata-cached = Hiçbir üst veri hizmeti yanıt vermedi. Son önbelleğe alınan veriler gösteriliyor.
notice-anilist-back = AniList yeniden yanıt veriyor.
notice-party-not-built = { -watch-party-cap-pl } bu uygulamaya dahil edilmemiş.
sources-demo-removed = Demo kaldırıldı
stores-ready-demo-stays = { $store } hazır. Demo, { -source-cap-pl }ınızın sonunda kalır.

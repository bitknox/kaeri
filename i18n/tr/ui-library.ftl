queue-episodes-left = { $count ->
    [one] { $title }: bir bölüm kaldı
   *[other] { $title }: { $count } bölüm kaldı
 }
episodes-count = { $count ->
    [one] { $count } bölüm
   *[other] { $count } bölüm
 }
progress-episodes = { $total ->
    [one] { $watched } / { $total } bölüm
   *[other] { $watched } / { $total } bölüm
 }
duration-days-hours = { $days } g { $hours } sa
group-in-progress = Devam eden
group-new-this-week = Bu hafta yeni
continue-empty-title = Henüz izlenen yok
continue-load-failed = { -continue-cap } yüklenemedi
continue-airing-today = Bugün yayınlananlar
continue-this-season = Bu sezon
continue-hide = { -continue-cap } listesinden gizle
continue-hidden = { -continue-cap } listesinden gizlendi.
continue-drop = Durumu Bırakıldı yap
continue-more-for = { $title } için diğer seçenekler
continue-row-resume = Devam et: E{ $episode } · { $clock }
continue-row-resume-dubbed = Devam et: E{ $episode } { -dubbed } · { $clock }
continue-row-start = Oynat: E{ $episode }
continue-row-start-dubbed = Oynat: E{ $episode } { -dubbed }
continue-row-start-aired = Oynat: E{ $episode } · yayınlandı: { $ago }
continue-row-caught-up = Yeni bölüm yok · E{ $episode }: { $when }
continue-row-caught-up-plain = Yeni bölüm yok
continue-widget-connect = Burada görmek için AniList'i bağlayın.
continue-caught-up = Yeni izlenecek bir şey kalmadı.
title-button-resume = Bölüm { $episode } devam et · { $time }
title-button-play = Bölüm { $episode } oynat
title-button-add-source = Oynatmak için bir { -source } ekleyin
library-status-watching = İzleniyor
library-status-planning = Planlandı
library-status-completed = Tamamlandı
library-status-paused = Ara verildi
library-status-dropped = Bırakıldı
library-sort-updated = Son güncelleme
library-sort-title = Başlık
library-sort-score = Puan
library-sort-button = Sıralama: { $sort }
library-sort-label = { $sort } ile sırala
library-signed-out-title = Listeleriniz AniList'ten gelir
library-signed-out-detail = İzlediklerinizi ve kaldığınız yeri eşitlemek için giriş yapın.
library-signed-out-action = AniList'e giriş yap
library-list-empty = { $list } listeniz boş
library-list-empty-detail = AniList'te bu listeye eklediğiniz başlıklar burada görünür.
library-list-load-failed = { $list } listeniz yüklenemedi
library-loading = Listeleriniz yükleniyor
library-add-to-list = Listeye ekle
library-add-planning = Planlamaya ekle
library-added-planning = Planlamaya eklendi.
library-on-your-list = Listenizde
library-in-your-list = Listenizde
progress-episodes-short = E{ $done } / { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Oynat: E{ $episode } { -dubbed } · yayınlandı: { $ago }
up-next-starts = Başlıyor: { $date }
up-next-reason-new-season = Yeni sezon
up-next-reason-planning = Planlama listenizde
up-next-reason-recommended = Bitirdiğiniz için: { $title }
up-next-reason-recommended-plain = Sizin için önerilen
up-next-because-detail = AniList'te { $title } beğenenler tarafından önerildi
up-next-add-planning = Planlamaya ekle
up-next-not-interested = İlgilenmiyorum
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title }, { -up-next-cap } listesinden gizlendi.
up-next-hidden-until-airs = { $title } yayına başlayana kadar gizlendi.
up-next-broadcast = Bölüm { $episode } · { $when }
up-next-out = yayınlandı: { $ago }
library-row-scanning = { $files ->
    [one] Taranıyor · 1 dosya bulundu
   *[other] Taranıyor · { $files } dosya bulundu
 }
library-row-matching = Eşleştiriliyor · { $done } / { $total } dizi
library-row-waiting = AniList bekleniyor · { $seconds } sn
library-row-summary = { $series } içinde { $episodes }
library-episodes = { $count ->
    [one] 1 bölüm
   *[other] { $count } bölüm
 }
library-series = { $count ->
    [one] 1 dizi
   *[other] { $count } dizi
 }
library-row-needs-look = { $count } tanesine bakmak gerek
library-row-drive-offline = Sürücü bağlı değil
library-row-missing = Klasör bulunamadı
library-row-no-access = { -app-name } bu klasörü artık açamıyor
library-locate = Bul…
library-choose-again = Yeniden seç
library-page-watching = Değişiklikler izleniyor · son tarama { $time }
library-page-no-watch = { -app-name } açılırken yeniden taranır · son tarama { $time }
library-rescan = Yeniden tara
library-show-explorer = Gezgin'de göster
library-remove-folder = Klasörü kaldır
library-needs-look = Bakılması gerekenler
library-not-matched = Eşleşmeyenler
library-matched = { $count } dizi eşleşti
library-best-guess = En iyi tahmin: { $title } · %{ $percent } eşleşme
library-picker-prompt = Klasör ekle
library-picker-message = Bölümlerinizin olduğu bir klasör seçin.
library-fix-confidence = %{ $percent } eşleşme
library-open-folder = Klasörü aç
library-matched-count = { $count } dizi eşleşti
library-row-series = { $count ->
    [one] 1 dizi
   *[other] { $count } dizi
 }
library-row-files = { $files ->
    [one] 1 dosya
   *[other] { $files } dosya
 }
library-show-finder = Finder'da göster
library-remove-line = Dosyalarınız olduğu yerde kalır.
library-files = { $count ->
    [one] 1 dosya
   *[other] { $count } dosya
 }
library-reason-no-episode = Adında bölüm numarası yok
library-reason-no-title = AniList'te başlık bulunamadı
library-reason-not-video = { -app-name } bu video dosyasını oynatamaz
library-set-episode = Bölüm ata…
library-keep-unmatched = Eşleşmemiş bırak
library-fix-title = Bu klasör hangi başlık?
library-reason-no-result = AniList'te başlık bulunamadı
library-kept = Olduğu gibi bırakıldı
library-fix-search = AniList'te ara
library-fix-hint = Bu dosyaların eşleşeceği başlığı arayın.
library-connect-drive = Oynatmak için sürücüyü bağlayın
library-fix-count-mismatch = { $titleEpisodes } bölümü var, bu klasörde { $files } dosya var
library-leave-unmatched = Eşleşmemiş bırak
library-episode-title = Bu dosya hangi bölüm?
library-season = Sezon
library-episode = Bölüm
library-apply = Uygula
library-from-files = Dosyalarından
title-connect-drive = Oynatmak için sürücüyü bağlayın
library-picker-title = Bölümlerinizin olduğu bir klasör seçin.
library-remove-detail = Dosyalarınız olduğu yerde kalır.
library-reason-offline = AniList'e sorulamadı. Sonraki taramada yeniden denenir.
library-choose-match = Eşleşmeyi seç
library-kept-line = Kendi başlığı olarak bırakıldı
library-fix-match = Bu eşleşmeyi düzelt
library-fix-none = Başlık bulunamadı
library-fix-offline = AniList'e ulaşılamadı. Çevrimiçi olunca yeniden deneyin.
library-removed = { $name } kaldırıldı
library-match-percent = %{ $percent } eşleşme

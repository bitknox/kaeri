progress-server-saved = { $server } sunucusuna kaydedildi
progress-server-sending = { $server } sunucusuna gönderiliyor…
progress-server-offline = Yeniden çevrimiçi olduğunuzda { $server } sunucusuna gönderilecek
progress-server-retrying = { $server } yanıt vermedi. { -app-name } yeniden deneyecek.
progress-server-failed = { $server } sunucusuna kaydedilmedi: { $reason }.
progress-why-key = sunucu anahtarı reddetti
progress-why-password = sunucu parolayı reddetti
progress-why-gone = bu seri artık sunucuda yok
progress-why-gave-up = bir gündür yanıt yok
progress-row-saved = İlerleme { $ago } kaydedildi
progress-row-sending = { $count ->
    [one] 1 bölüm gönderiliyor…
   *[other] { $count } bölüm gönderiliyor…
 }
progress-row-offline = { $count ->
    [one] 1 bölüm, yeniden çevrimiçi olduğunuzda gönderilecek
   *[other] { $count } bölüm, yeniden çevrimiçi olduğunuzda gönderilecek
 }
progress-row-retrying = { $count ->
    [one] 1 bölüm bekliyor. { -app-name } yeniden deneyecek.
   *[other] { $count } bölüm bekliyor. { -app-name } yeniden deneyecek.
 }
progress-row-failed = { $count ->
    [one] 1 bölüm kaydedilmedi: { $reason }
   *[other] { $count } bölüm kaydedilmedi: { $reason }
 }
progress-row-off = İlerleme bu { $kind } üzerinde kalır

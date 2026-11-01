queue-episodes-left = { $count ->
    [one] Ett avsnitt kvar i { $title }
   *[other] { $count } avsnitt kvar i { $title }
 }
episodes-count = { $count ->
    [one] { $count } avsnitt
   *[other] { $count } avsnitt
 }
progress-episodes = { $total ->
    [one] { $watched } av { $total } avsnitt
   *[other] { $watched } av { $total } avsnitt
 }
duration-days-hours = { $days } d { $hours } h
group-in-progress = Pågår
group-new-this-week = Nytt den här veckan
continue-empty-title = Inget sett än
continue-load-failed = Det gick inte att läsa { -continue-cap }
continue-airing-today = Sänds idag
continue-this-season = Den här säsongen
continue-hide = Dölj från { -continue-cap }
continue-hidden = Dold från { -continue-cap }.
continue-drop = Markera som avbruten
continue-more-for = Mer för { $title }
continue-row-resume = Fortsätt på E{ $episode } · { $clock }
continue-row-resume-dubbed = Fortsätt på E{ $episode } { -dubbed } · { $clock }
continue-row-start = Spela E{ $episode }
continue-row-start-dubbed = Spela E{ $episode } { -dubbed }
continue-row-start-aired = Spela E{ $episode } · släpptes { $ago }
continue-row-caught-up = Ikapp · E{ $episode } { $when }
continue-row-caught-up-plain = Ikapp
continue-widget-connect = Koppla AniList för att se det här.
continue-caught-up = Du är ikapp.
title-button-resume = Fortsätt E{ $episode } · { $time }
title-button-play = Spela avsnitt { $episode }
title-button-add-source = Lägg till en { -source } att spela från
library-status-watching = Tittar nu
library-status-planning = Planerar
library-status-completed = Klar
library-status-paused = Pausad
library-status-dropped = Avbruten
library-sort-updated = Senast uppdaterad
library-sort-title = Titel
library-sort-score = Betyg
library-sort-button = Sortera: { $sort }
library-sort-label = Sortera efter { $sort }
library-signed-out-title = Dina listor kommer från AniList
library-signed-out-detail = Logga in för att synka vad du tittar på och var du slutade.
library-signed-out-action = Logga in på AniList
library-list-empty = Listan { $list } är tom
library-list-empty-detail = Titlar du lägger till i den här listan på AniList visas här.
library-list-load-failed = Det gick inte att läsa in listan { $list }
library-loading = Läser in dina listor
library-add-to-list = Lägg till i lista
library-add-planning = Lägg till i planeringslistan
library-added-planning = Lade till i planeringslistan.
library-on-your-list = På din lista
library-in-your-list = I din lista
progress-episodes-short = E{ $done } av { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Spela E{ $episode } { -dubbed } · släpptes { $ago }
up-next-starts = Börjar { $date }
up-next-reason-new-season = Ny säsong
up-next-reason-planning = På din planeringslista
up-next-reason-recommended = Eftersom du slutade { $title }
up-next-reason-recommended-plain = Rekommenderas för dig
up-next-because-detail = Rekommenderas på AniList av personer som gillade { $title }
up-next-add-planning = Lägg till i planeringslistan
up-next-not-interested = Inte intresserad
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } är dold från { -up-next-cap } för dig
up-next-hidden-until-airs = { $title } är dold tills den börjar sändas
up-next-broadcast = Avsnitt { $episode } · { $when }
up-next-out = släpptes { $ago }
library-row-scanning = { $files ->
    [one] Söker igenom · 1 fil hittad
   *[other] Söker igenom · { $files } filer hittade
 }
library-row-matching = Matchar · { $done } av { $total } serier
library-row-waiting = Väntar på AniList · { $seconds } s
library-row-summary = { $episodes } från { $series }
library-episodes = { $count ->
    [one] 1 avsnitt
   *[other] { $count } avsnitt
 }
library-series = { $count ->
    [one] 1 serie
   *[other] { $count } serier
 }
library-row-needs-look = { $count } behöver en titt
library-row-drive-offline = Enheten är inte ansluten
library-row-missing = Mappen hittades inte
library-row-no-access = { -app-name } kan inte längre öppna den här mappen
library-locate = Leta upp…
library-choose-again = Välj igen
library-page-watching = Bevakas för ändringar · senaste genomsökning { $time }
library-page-no-watch = Söks igenom på nytt när { -app-name } startar · senaste genomsökning { $time }
library-rescan = Sök igen
library-show-explorer = Visa i Utforskaren
library-remove-folder = Ta bort mappen
library-needs-look = Behöver en titt
library-not-matched = Inte matchade
library-matched = { $count } serier matchade
library-best-guess = Bästa gissning: { $title } · { $percent } % match
library-picker-prompt = Lägg till mapp
library-picker-message = Välj en mapp med dina avsnitt.
library-fix-confidence = { $percent } % match
library-open-folder = Öppna mappen
library-matched-count = { $count } serier matchade
library-row-series = { $count ->
    [one] 1 serie
   *[other] { $count } serier
 }
library-row-files = { $files ->
    [one] 1 fil
   *[other] { $files } filer
 }
library-show-finder = Visa i Finder
library-remove-line = Dina filer ligger kvar där de är.
library-files = { $count ->
    [one] 1 fil
   *[other] { $count } filer
 }
library-reason-no-episode = Inget avsnittsnummer i namnet
library-reason-no-title = Ingen AniList-titel hittades
library-reason-not-video = Inte en videofil som { -app-name } kan spela
library-set-episode = Ange avsnitt…
library-keep-unmatched = Behåll omatchad
library-fix-title = Vilken titel är den här mappen?
library-reason-no-result = Ingen AniList-titel hittades
library-kept = Behålls som den är
library-fix-search = Sök på AniList
library-fix-hint = Sök efter titeln som filerna ska matchas mot.
library-connect-drive = Anslut enheten för att spela
library-fix-count-mismatch = Har { $titleEpisodes } avsnitt, mappen har { $files }
library-leave-unmatched = Lämna omatchad
library-episode-title = Vilket avsnitt är den här filen?
library-season = Säsong
library-episode = Avsnitt
library-apply = Använd
library-from-files = Från dina filer
title-connect-drive = Anslut enheten för att spela
library-picker-title = Välj en mapp med dina avsnitt.
library-remove-detail = Dina filer ligger kvar där de är.
library-reason-offline = Kunde inte fråga AniList. Det prövas igen vid en ny genomsökning.
library-choose-match = Välj matchning
library-kept-line = Behålls som en egen titel
library-fix-match = Rätta matchningen
library-fix-none = Inga titlar hittades
library-fix-offline = Kunde inte nå AniList. Försök igen när du är online.
library-removed = { $name } togs bort
library-match-percent = { $percent } % match

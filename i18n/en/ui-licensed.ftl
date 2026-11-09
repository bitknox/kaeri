## Licensed services: streaming services the viewer pays for, and the links to them.

# Title of the page.
licensed-title = Licensed services

# Line under the title.
licensed-body = { -app-name } can open titles in streaming services you pay for. It never plays them itself. Links come from AniList.

# Label of the region menu.
licensed-region = Region

# Line under the region menu.
licensed-region-line = Links AniList marks for other countries are hidden. Most links name no country, so choose your services too.

# Heading over the services the viewer pays for.
licensed-yours = Your services

# Row that shows the rest of the services.
# @arg count: int
licensed-show-all = Show all { $count }

# Choice in the Region picker that follows the device's own country.
licensed-region-system = Follow this { -device }

# Value of the region choice before one is picked: the device's own.
licensed-region-device = This { -device }'s region

# Switch: name services the person doesn't use on title pages.
licensed-show-others = Show other services on title pages

# Line under that switch.
licensed-show-others-line = A line under the buttons names services you don't use.

# Line of the Sources row: the ticked services and the region.
# @arg services: text
# @arg region: text
licensed-row-line = { $services } · { $region }

# Line of that row before any service is chosen.
licensed-row-none = Choose the services you pay for

# Line while AniList's list of services has not arrived.
licensed-loading = Asking AniList for its list of services

# Main action of a title that does not play here but is on a service. The service is one the person uses.
# @arg service: text
# @max 32
title-watch-on = Watch on { $service }

# Line under the actions of a title that plays here and on a service.
# @arg services: text
title-also-on-services = Also on { $services }

# Line on a title found in none of the viewer's services.
# @arg services: text
title-listed-on = Not found in your { -source-pl }. Listed on { $services }.

# Link after that line.
title-choose-services = Choose your services

# Button on the return notice.
# @max 24
licensed-mark-watched = Mark watched

# Menu item that opens a title in one of the viewer's services.
# @arg service: text
title-watch-on-item = Watch on { $service }

# Button that undoes marking as watched.
licensed-mark-undo = Undo

# Tooltip on the glyph that says a row opens outside the app.
licensed-opens-outside = Opens in your browser

# Line under the buttons of a title that is also listed in services the viewer does not use.
# @arg services: text
title-others-line = Also listed on { $services }

# Main button of a title that does not play here but is on a service the viewer uses.
# @arg service: text
# @max 32
licensed-watch-on = Watch on { $service }

# Line under a title's buttons naming the viewer's other services that have it.
# @arg services: text
licensed-also-on = Also on { $services }

# Line on a title found in none of the viewer's sources, naming the services that list it.
# @arg services: text
licensed-listed-on = Not found in your { -source-pl }. Listed on { $services }.

# Link on a title page that opens the Licensed services page.
licensed-choose-services = Choose your services

# Line of a Continue card for a title watched on a service.
# @arg episode: plain
# @arg service: text
licensed-continue-line = Episode { $episode } · on { $service }

# Line under a title's buttons naming services the viewer has not chosen that list the title.
# @arg services: text
licensed-others = Listed on { $services }.

# Heading of the panel the TV shows when no app handles a service's link.
# @arg service: text
licensed-tv-not-installed = { $service } isn't installed on this TV.

# Label over the QR code on that panel.
licensed-tv-open-phone = Open on your phone

# Button on that panel when a phone is paired.
# @max 24
licensed-tv-send = Send to phone

# Note on that panel after Send to phone reached a connected phone.
licensed-tv-sent = Sent. Your phone opens it now.

# Note on that panel after Send to phone, when no paired phone is connected.
licensed-tv-held = No phone is connected. The first one to connect in the next two minutes opens it.

# Note on that panel when the link could not be sent.
licensed-tv-send-failed = That link can't be sent.

# Accessibility label of the glyph that says a button opens another app.
licensed-opens-service = Opens in another app

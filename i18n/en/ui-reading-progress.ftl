## Did my server get it? The line on the page between chapters and the server's row in Your
## sources say what happened to the progress sent back to a Komga or Kavita server.

# Page between chapters, under the tracker line: the server has the progress.
# @arg server: text
progress-server-saved = Saved to { $server }

# Same place, only after 600 ms without an answer.
# @arg server: text
progress-server-sending = Sending to { $server }…

# Same place, offline: the send waits in the queue.
# @arg server: text
progress-server-offline = Will send to { $server } when you’re back online

# Same place, the send failed for a reason that can pass.
# @arg server: text
progress-server-retrying = { $server } didn’t answer. { -app-name } will try again.

# Same place, the server said no. Try again follows it.
# @arg server: text
# @arg reason: text
progress-server-failed = Not saved to { $server }: { $reason }.

# Reason: a Kavita key.
progress-why-key = the server refused the key

# Reason: a Komga or OPDS password.
progress-why-password = the server refused the password

# Reason.
progress-why-gone = this series isn’t on the server any more

# Reason, after a day of retries.
progress-why-gave-up = no answer for a day

# Your sources, the server's row, second line.
# @arg ago: text
progress-row-saved = Progress saved { $ago }

# The server's row, after 600 ms without an answer.
# @arg count: int
progress-row-sending = { $count ->
    [one] Sending 1 chapter…
   *[other] Sending { $count } chapters…
 }

# The server's row, offline.
# @arg count: int
progress-row-offline = { $count ->
    [one] 1 chapter will send when you’re back online
   *[other] { $count } chapters will send when you’re back online
 }

# The server's row, the last try failed for a reason that can pass.
# @arg count: int
progress-row-retrying = { $count ->
    [one] 1 chapter waiting. { -app-name } will try again.
   *[other] { $count } chapters waiting. { -app-name } will try again.
 }

# The server's row, the server said no. Try again sits beside it.
# @arg count: int
# @arg reason: text
progress-row-failed = { $count ->
    [one] 1 chapter not saved: { $reason }
   *[other] { $count } chapters not saved: { $reason }
 }

# The server's row, when Send progress to the server is off.
progress-row-off = Progress stays on this { -device }

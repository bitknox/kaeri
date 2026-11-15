source-picker-seeders = { $count ->
    [one] { $count } сидер
    [few] { $count } сидера
    [many] { $count } сидеров
   *[other] { $count } сидера
 }
rate-per-second = { $size }/с
app-name = { -app-name }

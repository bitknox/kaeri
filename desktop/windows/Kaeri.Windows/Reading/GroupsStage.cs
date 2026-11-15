#if KAERI_TESTING
using Kaeri.App;
using Kaeri.Engine.Model;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Shots: a source with several groups, staged on the fixtures engine, whose editions name none.
/// The series page then has a Group row and its picker, and the chapter rows name a chapter from
/// another group, one read in two parts and a part from a group in another language. Names are
/// made up, the same as the Android and Linux stages.
/// </summary>
static class GroupsStage
{
    public static void Stage(ReadingStore reading)
    {
        reading.EditionShot = WithGroups;
        reading.VolumesShot = WithParts;
    }

    static BookEditionRow WithGroups(BookEditionRow edition)
    {
        var total = Math.Max(edition.Chapters, 20u);
        return edition with
        {
            VersionLabel = "Lanternlight Scans", // i18n-skip
            Groups =
            [
                new GroupChoice("Lanternlight Scans", total - 2, InUse: true, Language: "en"), // i18n-skip
                new GroupChoice("Saltmarsh Team", total * 3 / 5, Language: "en"), // i18n-skip
                new GroupChoice("Bluefinch Group", total / 4, Language: "es"), // i18n-skip
            ],
            GroupChosen = false,
        };
    }

    static VolumeGroup[] WithParts(VolumeGroup[] volumes)
    {
        var seen = 0;
        return [.. volumes.Select(volume => volume with
        {
            Chapters = [.. volume.Chapters.Select(chapter =>
            {
                var row = (seen++) switch
                {
                    0 => chapter with
                    {
                        OtherGroup = true,
                        VersionLabel = "Saltmarsh Team", // i18n-skip
                        Line = [.. chapter.Line ?? [], new Msg.Reading(new ReadingMsg.ReadingRowFromGroup("Saltmarsh Team"))], // i18n-skip
                    },
                    2 => chapter with
                    {
                        Parts = 2,
                        Line = [.. chapter.Line ?? [], new Msg.Reading(new ReadingMsg.ReadingRowParts(2))],
                    },
                    4 => chapter with
                    {
                        Parts = 2,
                        PartGroups = [new PartGroup(2, "Bluefinch Group", "es")], // i18n-skip
                        Line = [.. chapter.Line ?? [], new Msg.Reading(new ReadingMsg.ReadingRowParts(2)),
                            new Msg.Reading(new ReadingMsg.ReadingRowPartFromGroupIn(2, "Bluefinch Group", "es"))], // i18n-skip
                    },
                    _ => chapter,
                };
                return row;
            })],
        })];
    }
}
#endif

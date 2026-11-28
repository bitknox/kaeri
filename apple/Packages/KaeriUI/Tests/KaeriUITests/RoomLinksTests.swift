// A room's invite links: the app scheme and the web address open the room; the party's and other
// pages don't.

@testable import KaeriUI
import Foundation
import Testing

struct RoomLinksTests {
    @Test func aRoomsLinksAreInvites() throws {
        #expect(RoomLinks.isInvite(try #require(URL(string: "kaeri://room#v=3.Q2m4xR9dK7tW"))))
        #expect(RoomLinks.isInvite(try #require(URL(string: "https://kaeri.app/room#v=3.Q2m4xR9dK7tW"))))
        #expect(RoomLinks.isInvite(try #require(URL(string: "HTTPS://Kaeri.app/Room#v=3.Q2m4xR9dK7tW"))))
    }

    @Test func otherLinksAreNot() throws {
        #expect(!RoomLinks.isInvite(try #require(URL(string: "kaeri://party?room=abc"))))
        #expect(!RoomLinks.isInvite(try #require(URL(string: "https://kaeri.app/party#v1.abc"))))
        #expect(!RoomLinks.isInvite(try #require(URL(string: "https://kaeri.app/legal"))))
        #expect(!RoomLinks.isInvite(try #require(URL(string: "https://room.example/room#v=3.abc"))))
        #expect(!RoomLinks.isInvite(try #require(URL(string: "kaeri://pair/042917@192.168.1.20:4510"))))
    }
}

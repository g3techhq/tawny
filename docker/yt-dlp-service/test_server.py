import unittest

import server


class CommandTests(unittest.TestCase):
    def test_command_has_token_provider_and_supported_js_runtime(self) -> None:
        command = server.build_video_command("aNXB-8Aqt88")
        self.assertIn("node", command)
        self.assertIn("youtube:player_client=mweb", command)
        self.assertTrue(
            any(value.startswith("youtubepot-bgutilhttp:base_url=") for value in command)
        )

    def test_command_rejects_non_video_ids(self) -> None:
        with self.assertRaises(ValueError):
            server.build_video_command("../../secrets")

    def test_shorts_command_is_flat_and_bounded(self) -> None:
        command = server.build_channel_shorts_command("UCsXVk37bltHxD1rDPwtNM8Q")
        self.assertIn("--flat-playlist", command)
        self.assertIn("30", command)
        self.assertTrue(command[-1].endswith("/shorts"))

    def test_shorts_command_rejects_non_channel_ids(self) -> None:
        with self.assertRaises(ValueError):
            server.build_channel_shorts_command("veritasium")


if __name__ == "__main__":
    unittest.main()

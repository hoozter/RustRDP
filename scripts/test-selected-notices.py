#!/usr/bin/env python3
"""Check the selected crate notices against reviewed release-file hashes.

This does not establish full graph coverage or historical artifact compliance.
"""
import hashlib
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
# Hashes from upstream crate archives verified against Cargo.lock checksums.
RECORDS = [{'name': 'cfg-if', 'version': '1.0.4', 'file': 'LICENSE-MIT', 'archive_sha256': '9330f8b2ff13f34540b44e946ef35111825727b38d33286ef986142615121801', 'text_sha256': '378f5840b258e2779c39418f3f2d7b2ba96f1c7917dd6be0713f88305dbda397'}, {'name': 'libc', 'version': '0.2.189', 'file': 'LICENSE-MIT', 'archive_sha256': '3eaf3ede3fee6db1a4c2ee091bf8a8b4dccdc6d17f656fb07896ee72867612f2', 'text_sha256': '123a331b5dbf04c30097fa43b8f858bc85df671fe776de498d01f3d6b7c1f69e'}, {'name': 'log', 'version': '0.4.34', 'file': 'LICENSE-MIT', 'archive_sha256': 'f9f8bd3e56ce4dfc153cf470fffbfa98c7620958b312ca5c3a4b8d5181fd13c6', 'text_sha256': '6485b8ed310d3f0340bf1ad1f47645069ce4069dcc6bb46c7d5c6faf41de1fdb'}, {'name': 'memchr', 'version': '2.8.3', 'file': 'LICENSE-MIT', 'archive_sha256': 'cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98', 'text_sha256': '0f96a83840e146e43c0ec96a22ec1f392e0680e6c1226e6f3ba87e0740af850f'}, {'name': 'memchr', 'version': '2.8.3', 'file': 'COPYING', 'archive_sha256': 'cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98', 'text_sha256': '01c266bced4a434da0051174d6bee16a4c82cf634e2679b6155d40d75012390f'}, {'name': 'once_cell', 'version': '1.21.4', 'file': 'LICENSE-MIT', 'archive_sha256': '9f7c3e4beb33f85d45ae3e3a1792185706c8e16d043238c593331cc7cd313b50', 'text_sha256': '23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3'}, {'name': 'serde', 'version': '1.0.229', 'file': 'LICENSE-MIT', 'archive_sha256': '4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba', 'text_sha256': '23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3'}, {'name': 'serde_core', 'version': '1.0.229', 'file': 'LICENSE-MIT', 'archive_sha256': '67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48', 'text_sha256': '23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3'}, {'name': 'serde_derive', 'version': '1.0.229', 'file': 'LICENSE-MIT', 'archive_sha256': 'e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348', 'text_sha256': '23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3'}]


class SelectedNoticeTests(unittest.TestCase):
    def test_original_texts_and_locked_sources(self):
        notices = (ROOT / 'THIRD_PARTY_NOTICES.md').read_text()
        packages = {(p['name'], p['version']): p for p in
                    tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']}
        for record in RECORDS:
            with self.subTest(package=record['name'], file=record['file']):
                heading = f"### {record['name']} {record['version']} — {record['file']}"
                sections = notices.split(heading + '\n')
                self.assertEqual(len(sections), 2)
                section = sections[1].split('\n### ', 1)[0]
                text = re.search(r'````text\n(.*?)````', section, re.S)
                if text is None:
                    self.fail('Missing fenced original license text')
                self.assertEqual(hashlib.sha256(text[1].encode()).hexdigest(),
                                 record['text_sha256'])
                self.assertEqual(packages[record['name'], record['version']]['checksum'],
                                 record['archive_sha256'])
                self.assertIn(record['archive_sha256'], section)


if __name__ == '__main__':
    unittest.main()

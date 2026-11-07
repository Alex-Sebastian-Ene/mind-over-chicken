import unittest


def add(a, b):
    return a + b


class SmokeTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 3), 5)

    def test_add_negative(self):
        self.assertEqual(add(-2, 3), 1)

    def test_add_floats(self):
        self.assertAlmostEqual(add(0.1, 0.2), 0.3)


if __name__ == "__main__":
    unittest.main()

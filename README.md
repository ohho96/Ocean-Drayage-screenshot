# Web Screenshot Tool

A Python application that uses Playwright to take screenshots of webpages.

## Setup

1. Install Python dependencies:
```bash
pip install -r requirements.txt
```

2. Install Playwright browsers:
```bash
playwright install
```

## Usage

Basic usage:
```bash
python screenshot.py https://example.com
```

Options:
- `--output-dir`: Specify the directory to save screenshots (default: "screenshots")
- `--no-full-page`: Take only viewport screenshot instead of full page

Example:
```bash
python screenshot.py https://example.com --output-dir my_screenshots --no-full-page
```

Screenshots will be saved with timestamps in the format: `screenshot_YYYYMMDD_HHMMSS.png`

Terminal Screenshot

Appointment screennshot

E-Modal

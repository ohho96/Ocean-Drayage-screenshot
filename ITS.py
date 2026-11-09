from playwright.sync_api import sync_playwright
import os
from datetime import datetime

# Terminal configuration
TERMINAL_URL = "https://www.itslb.com/empty-receiving-updates/"  # Replace with actual ITS URL

def take_screenshot(url=TERMINAL_URL, output_dir="terminal_screenshots", full_page=True):
    """
    Take a screenshot of a webpage using Playwright.
    
    Args:
        url (str): The URL of the webpage to screenshot
        output_dir (str): Directory to save the screenshot
        full_page (bool): Whether to take a full page screenshot
    """
    # Create output directory if it doesn't exist
    os.makedirs(output_dir, exist_ok=True)
    
    # Generate filename with timestamp
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    filename = f"{output_dir}/screenshot_{timestamp}.png"
    
    with sync_playwright() as p:
        # Launch browser
        browser = p.chromium.launch()
        page = browser.new_page()
        
        try:
            # Navigate to URL
            print(f"Navigating to {url}...")
            page.goto(url, wait_until="networkidle")
            
            # Take screenshot
            print(f"Taking screenshot...")
            page.screenshot(path=filename, full_page=full_page)
            print(f"Screenshot saved to {filename}")
            
        except Exception as e:
            print(f"Error occurred: {str(e)}")
        
        finally:
            browser.close() 
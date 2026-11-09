from playwright.sync_api import sync_playwright, TimeoutError, Error as PlaywrightError
import os
from datetime import datetime
from dotenv import load_dotenv
import sys

# Load environment variables
load_dotenv()

# Terminal configuration
TERMINAL_URL = "https://portal.lbct.com/CargoSearch"  # Updated LBCT URL
TERMINAL_URL_LOGIN = "https://portal.lbct.com/LoginWithUrl" # Login URL
TERMINAL_URL_ViewMyList = "https://portal.lbct.com/ViewMyList" # pager after login
LBCT_ID = os.getenv('LBCT_ID')
LBCT_PASSWORD = os.getenv('LBCT_PASSWORD')

def handle_error(page, error, error_type="unknown"):
    """
    Handle errors by taking a screenshot and printing detailed information.
    
    Args:
        page: Playwright page object
        error: The error that occurred
        error_type: Type of error for the screenshot filename
    """
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    error_filename = f"screenshots/lbct_{error_type}_error_{timestamp}.png"
    
    try:
        page.screenshot(path=error_filename, full_page=True)
        print(f"Error screenshot saved to {error_filename}")
    except Exception as screenshot_error:
        print(f"Failed to take error screenshot: {str(screenshot_error)}")
    
    print(f"\nError Details:")
    print(f"Type: {error_type}")
    print(f"Message: {str(error)}")
    print(f"Time: {timestamp}")

def take_screenshot(url=TERMINAL_URL, output_dir="terminal_screenshots", full_page=True, browser=None, page=None):
    """
    Take a screenshot of a webpage using Playwright.
    
    Args:
        url (str): The URL of the webpage to screenshot
        output_dir (str): Directory to save the screenshot
        full_page (bool): Whether to take a full page screenshot
        browser: Optional existing browser instance
        page: Optional existing page instance
    """
    # Create output directory if it doesn't exist
    os.makedirs(output_dir, exist_ok=True)
    
    # Generate filename with timestamp
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    filename = f"{output_dir}/lbct_screenshot_{timestamp}.png"
    
    should_close_browser = False
    try:
        # Use existing browser/page if provided, otherwise create new ones
        if not browser or not page:
            p = sync_playwright().start()
            browser = p.chromium.launch(headless=False)
            page = browser.new_page()
            should_close_browser = True
            
        # Navigate to URL
        print(f"Navigating to {url}...")
        page.goto(url, wait_until="networkidle", timeout=60000)
        
        # Take screenshot
        print(f"Taking screenshot...")
        page.screenshot(path=filename, full_page=full_page)
        print(f"Screenshot saved to {filename}")
        
    except TimeoutError as e:
        handle_error(page, e, "timeout")
        print("Navigation timed out. Please check your internet connection.")
    except PlaywrightError as e:
        handle_error(page, e, "playwright")
        print("A Playwright error occurred. Please check the browser automation.")
    except Exception as e:
        handle_error(page, e, "unexpected")
        print("An unexpected error occurred.")
    finally:
        # Only close the browser if we created it
        if should_close_browser and browser:
            browser.close()
            
    return browser, page

def LBCT_login(browser=None, page=None):
    """
    Login to LBCT portal and return browser and page objects for reuse.
    Handles both cases - with and without captcha.
    
    Args:
        browser: Optional existing browser instance
        page: Optional existing page instance
    
    Returns:
        tuple: (browser, page) objects if successful, (None, None) if failed
    """
    if not LBCT_ID or not LBCT_PASSWORD:
        print("Error: LBCT credentials not found in .env file")
        print("Please make sure LBCT_ID and LBCT_PASSWORD are set in your .env file")
        return None, None

    should_close_browser = False
    try:
        # Use existing browser/page if provided, otherwise create new ones
        if not browser or not page:
            p = sync_playwright().start()
            browser = p.chromium.launch(headless=False)
            page = browser.new_page()
            should_close_browser = True
        
        # Navigate to login URL
        print(f"Navigating to {TERMINAL_URL_LOGIN}...")
        try:
            page.goto(TERMINAL_URL_LOGIN, wait_until="networkidle")
        except TimeoutError:
            handle_error(page, "Navigation timeout", "navigation")
            if should_close_browser and browser:
                browser.close()
            return None, None
        
        # Login 
        print("Logging in to LBCT...")
        try:
            # Look for login form
            print("Waiting for login form...")
            username_field = page.wait_for_selector('input.loginBoxInputUser', state='visible')
            if username_field:
                print("Login form found, entering credentials...")
                print(f"Attempting to fill username: {LBCT_ID}")
                # Enter credentials with explicit waits
                page.wait_for_selector('input.loginBoxInputUser', state='visible')
                page.fill('input.loginBoxInputUser', LBCT_ID)
                print("Username filled")
                
                print(f"Attempting to fill password: {'*' * len(LBCT_PASSWORD)}")
                page.wait_for_selector('input.loginBoxInputPassword', state='visible')
                page.fill('input.loginBoxInputPassword', LBCT_PASSWORD)
                print("Password filled")
                
                # Check if reCAPTCHA is present
                try:
                    # First check for the reCAPTCHA container
                    recaptcha_container = page.wait_for_selector('div.g-recaptcha', state='visible', timeout=5000)
                    if recaptcha_container:
                        print("reCAPTCHA found. Please solve it and click the login button...")
                        # Wait for the reCAPTCHA iframe to be ready
                        recaptcha_iframe = page.wait_for_selector('iframe[title="reCAPTCHA"]', state='visible', timeout=5000)
                        if not recaptcha_iframe:
                            print("reCAPTCHA iframe not found, but container is present. Proceeding with login...")
                except TimeoutError:
                    print("No reCAPTCHA found, proceeding with login...")
                
                # Wait for login button and click it
                print("Waiting for login button...")
                login_button = page.wait_for_selector('#loginBoxLogin', state='visible')
                if login_button:
                    print("Clicking login button...")
                    login_button.click()
                
                # Wait for login completion
                print("Waiting for login completion...")
                try:
                    # Wait for navigation to container search page
                    page.wait_for_url(TERMINAL_URL_ViewMyList, timeout=300000)  # 5 minutes timeout
                    print("Successfully logged in!")
                    return browser, page
                except TimeoutError:
                    print("Login timeout. Please check if login was completed correctly.")
                    if should_close_browser and browser:
                        browser.close()
                    return None, None
            else:
                print("Login form not found")
                if should_close_browser and browser:
                    browser.close()
                return None, None
                
        except TimeoutError as e:
            handle_error(page, e, "login_timeout")
            print("Login form not found within timeout period")
            if should_close_browser and browser:
                browser.close()
            return None, None
        except Exception as e:
            handle_error(page, e, "login")
            print(f"Error during login: {str(e)}")
            if should_close_browser and browser:
                browser.close()
            return None, None
            
    except PlaywrightError as e:
        handle_error(page, e, "playwright")
        print("A Playwright error occurred. Please check the browser automation.")
        if should_close_browser and browser:
            browser.close()
        return None, None
    except Exception as e:
        handle_error(page, e, "unexpected")
        print("An unexpected error occurred.")
        if should_close_browser and browser:
            browser.close()
        return None, None

def keep_browser_open(browser, page):
    """
    Keep the browser open and handle various exit scenarios.
    """
    try:
        while True:
            try:
                # Check if browser is still open
                if not browser.is_connected():
                    print("\nBrowser was closed. Exiting...")
                    break
                    
                # Wait a bit before checking again
                page.wait_for_timeout(1000)
                
            except Exception as e:
                if "Target page, context or browser has been closed" in str(e):
                    print("\nBrowser was closed. Exiting...")
                    break
                else:
                    print(f"\nUnexpected error: {str(e)}")
                    break
                    
    except KeyboardInterrupt:
        print("\nReceived keyboard interrupt. Closing browser...")
    finally:
        try:
            if browser and browser.is_connected():
                browser.close()
        except:
            pass
        print("Browser closed.")

def check_container_return_availability(container_number="FFAU694237"):
    """
    Check container information on LBCT portal.
    
    Args:
        container_number (str): The container number to check
    """
    if not LBCT_ID or not LBCT_PASSWORD:
        print("Error: LBCT credentials not found in .env file")
        print("Please make sure LBCT_ID and LBCT_PASSWORD are set in your .env file")
        return

    # First login and get browser/page objects
    browser, page = LBCT_login()
    if not browser or not page:
        print("Failed to login. Exiting...")
        return

    try:
        # Wait for the container number input field
        print("Looking for container number input...")
        try:
            container_input = page.wait_for_selector('input[name="containerNumber"]', state='visible', timeout=10000)
            if container_input:
                print("Found container input, entering container number...")
                # Clear the input first
                page.fill('input[name="containerNumber"]', '')
                page.wait_for_timeout(1000)
                
                # Enter the container number
                page.fill('input[name="containerNumber"]', container_number)
                page.wait_for_timeout(2000)
                
                # Click search button
                print("Clicking search button...")
                search_button = page.wait_for_selector('button[type="submit"]', state='visible', timeout=10000)
                if search_button:
                    page.click('button[type="submit"]')
                    
                    # Wait for results
                    print("Waiting for search results...")
                    page.wait_for_load_state("networkidle", timeout=60000)
                    
                    # Take screenshot of results
                    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
                    filename = f"screenshots/lbct_container_{container_number}_{timestamp}.png"
                    page.screenshot(path=filename, full_page=True)
                    print(f"Results screenshot saved to {filename}")
                else:
                    handle_error(page, "Search button not found", "search_button")
            else:
                handle_error(page, "Container input not found", "container_input")
        except TimeoutError as e:
            handle_error(page, e, "container_search")
            print("Container search timed out. Please try again.")
        except Exception as e:
            handle_error(page, e, "container_search")
            print("An error occurred during container search.")
            
    except PlaywrightError as e:
        handle_error(page, e, "playwright")
        print("A Playwright error occurred. Please check the browser automation.")
    except Exception as e:
        handle_error(page, e, "unexpected")
        print("An unexpected error occurred.")
    finally:
        if browser:
            print("Waiting 5 seconds before closing...")
            page.wait_for_timeout(5000)
            browser.close()

if __name__ == "__main__":
    print("Starting LBCT login...")
    browser, page = LBCT_login()
    if browser and page:
        print("Login successful! Browser will remain open for further operations.")
        print("Press Ctrl+C to close the browser gracefully.")
        keep_browser_open(browser, page)
    else:
        print("Login failed. Exiting...")
import argparse
from ITS import take_screenshot as ITS_screenshot
from LBCT import take_screenshot as LBCT_screenshot
from APM import take_screenshot as APM_screenshot

def main():
    parser = argparse.ArgumentParser(description="Take screenshots of webpages using Playwright")
    parser.add_argument("--output-dir", default="screenshots", help="Directory to save screenshots")
    parser.add_argument("--no-full-page", action="store_true", help="Take only viewport screenshot")
    
    args = parser.parse_args()
    
    # Take screenshot using ITS
    print("\nTaking screenshot using ITS...")
    ITS_screenshot(
        output_dir=f"{args.output_dir}/ITS",
        full_page=not args.no_full_page
    )
    
    # Take screenshot using LBCT
    print("\nTaking screenshot using LBCT...")
    LBCT_screenshot(
        output_dir=f"{args.output_dir}/LBCT",
        full_page=not args.no_full_page
    )
    
    # Take screenshot using APM
    print("\nTaking screenshot using APM...")
    APM_screenshot(
        output_dir=f"{args.output_dir}/APM",
        full_page=not args.no_full_page
    )

if __name__ == "__main__":
    main() 
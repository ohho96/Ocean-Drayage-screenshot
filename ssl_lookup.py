"""
Shipping Line SCAC Code Lookup Module

This module provides functions to convert between shipping line names and their SCAC codes.
SCAC (Standard Carrier Alpha Code) is a unique code assigned to transportation companies.

Example:
    >>> get_scac("Maersk")
    'MAEU'
    >>> get_ssl("MAEU")
    'Maersk'
"""

from typing import Dict, Optional

# Dictionary mapping shipping line names to their SCAC codes
ssl_to_scac: Dict[str, str] = {
    # HMM / Hyundai
    "Hyundai Merchant Marine": "HDMU",
    "HMM": "HDMU",

    # Maersk & Subsidiaries
    "Maersk Line": "MAEU",
    "Maersk": "MAEU",
    "Sealand": "MAEU",
    "Hamburg Süd": "MAEU",  # Owned by Maersk

    # MSC
    "MSC": "MSCU",
    "Mediterranean Shipping Company": "MSCU",

    # CMA CGM Group
    "CMA CGM": "CMDU",
    "APL": "CMDU",  # APL is a CMA subsidiary
    "ANL": "CMDU",  # Also CMA group

    # COSCO & Group
    "COSCO": "COSU",
    "China Ocean Shipping Company": "COSU",
    "OOCL": "COSU",  # COSCO owns OOCL

    # Evergreen
    "Evergreen Marine": "EGLV",
    "Evergreen": "EGLV",

    # Hapag-Lloyd
    "Hapag-Lloyd": "HLCU",
    "HLAG": "HLCU",  # Hapag SCAC in some port systems

    # ONE
    "ONE": "ONEY",
    "Ocean Network Express": "ONEY",

    # Yang Ming
    "Yang Ming": "YMLU",

    # ZIM
    "ZIM": "ZIMU",
    "ZIM Integrated Shipping": "ZIMU",

    # WAN HAI
    "Wan Hai": "WHLC",
    "Wan Hai Lines": "WHLC",

    # PIL
    "PIL": "PABV",
    "Pacific International Lines": "PABV",

    # SM Line
    "SM Line": "SMLU",
    "SM": "SMLU",

    # Seaboard Marine
    "Seaboard Marine": "SMLM",

    # Matson
    "Matson": "MATS",
    "Matson Navigation": "MATS",

    # Swire Shipping
    "Swire": "CHVW",
    "Swire Shipping": "CHVW",

    # ACL (Atlantic Container Line)
    "ACL": "ACLU",
    "Atlantic Container Line": "ACLU",

    # ARKAS
    "Arkas": "ARKU",

    # Additional carriers
    "K Line": "KLNE",
    "Kawasaki Kisen Kaisha": "KLNE",
    "MOL": "MOLU",
    "Mitsui O.S.K. Lines": "MOLU",
    "NYK": "NYKU",
    "Nippon Yusen Kaisha": "NYKU",
    "TS Lines": "TSLU",
    "TS": "TSLU",
    "X-Press Feeders": "XPRF",
    "X-Press": "XPRF"
}

# Dictionary mapping SCAC codes to shipping line names
scac_to_ssl: Dict[str, str] = {
    "MAEU": "Maersk",
    "SEAU": "Maersk",  # Sealand SCAC
    "HDMU": "Hyundai Merchant Marine",
    "CMDU": "CMA CGM",
    "APLU": "CMA CGM",  # APL
    "ANNU": "CMA CGM",  # ANL
    "COSU": "COSCO",
    "OOLU": "COSCO",  # OOCL
    "HLCU": "Hapag-Lloyd",
    "HLAG": "Hapag-Lloyd",
    "MSCU": "MSC",
    "ONEY": "ONE",
    "EGLV": "Evergreen",
    "YMLU": "Yang Ming",
    "WHLC": "Wan Hai",
    "SMLU": "SM Line",
    "MATS": "Matson",
    "KLNE": "K Line",
    "MOLU": "MOL",
    "NYKU": "NYK",
    "TSLU": "TS Lines",
    "XPRF": "X-Press Feeders"
}

def get_scac(ssl_name: str) -> str:
    """
    Convert a shipping line name to its SCAC code.

    Args:
        ssl_name (str): The name of the shipping line

    Returns:
        str: The SCAC code for the shipping line, or "Unknown SCAC" if not found

    Example:
        >>> get_scac("Maersk")
        'MAEU'
    """
    if not ssl_name or not isinstance(ssl_name, str):
        return "Unknown SCAC"
    
    name = ssl_name.strip().title()
    return ssl_to_scac.get(name, "Unknown SCAC")

def get_ssl(scac_name: str) -> str:
    """
    Convert a SCAC code to its shipping line name.

    Args:
        scac_name (str): The SCAC code of the shipping line

    Returns:
        str: The name of the shipping line, or "Unknown SSL" if not found

    Example:
        >>> get_ssl("MAEU")
        'Maersk'
    """
    if not scac_name or not isinstance(scac_name, str):
        return "Unknown SSL"
    
    name = scac_name.strip().upper()
    return scac_to_ssl.get(name, "Unknown SSL")

if __name__ == "__main__":
    # Example usage
    print("Testing SCAC lookup:")
    print(f"Maersk -> {get_scac('Maersk')}")
    print(f"CMA CGM -> {get_scac('CMA CGM')}")
    
    print("\nTesting SSL lookup:")
    print(f"MAEU -> {get_ssl('MAEU')}")
    print(f"CMDU -> {get_ssl('CMDU')}")